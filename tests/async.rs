#[macro_use]
extern crate serde;
extern crate serde_json;
extern crate async_std;
extern crate futures;
extern crate tokio;
extern crate reqwest;
extern crate async_finite_state_machine;

use std::fs::File;
use std::io::Read;

use reqwest::Client;

use serde::{ Serialize, Deserialize };
use std::pin::Pin;
use std::fmt;
use std::error::Error;
use async_finite_state_machine::{ AsyncMachine };
use futures::task;

use {
    futures::{
        future::{FutureExt, BoxFuture},
        task::{ArcWake, waker_ref},
    },
    std::{
        future::Future,
        sync::{Arc, Mutex},
        sync::mpsc::{sync_channel, SyncSender, Receiver},
        task::{Context, Poll},
        time::Duration,
    },
};

#[test]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> { 
  struct StateFuture {
    inner: Pin<Box<dyn Future<Output = Result<State, Failures>> + Send>>,
  }
  impl StateFuture {
      fn new(fut: Box<dyn Future<Output = Result<State, Failures>> + Send>) -> Self {
          Self { inner: fut.into() }
      }
  }

  impl fmt::Debug for StateFuture {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          f.pad("Future<State>")
      }
  }

  impl Future for StateFuture {
      type Output = Result<State, Failures>;

      fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
          Pin::new(&mut self.inner).poll(cx)
      }
  }
  #[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
  struct Friend {
      id: String,
      name: String,
  }
  #[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
  struct User {
      id: String,
      eyeColor: String,
      name: String,
      company: String,
      email: String,
      friends: Vec<Friend>,
  }
  #[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
  struct State {
      users: Vec<User>,
  };
  #[derive(Debug, Clone, PartialEq)]
  enum Events {
      GetUsers,
  };
  #[derive(Debug, Clone, PartialEq)]
  enum States {
      Loading(State),
      Done(State),
  };
  #[derive(Debug, Clone, PartialEq)]
  enum Failures {
      Message(String),
  };

  impl std::fmt::Display for Failures {
      fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
          match self {
              Failures::Message(s) => write!(f, "State Failure: {}", s)
          }
      }
  }

  // This is important for other errors to wrap this one.
  impl Error for Failures {
      fn source(&self) -> Option<&(dyn Error + 'static)> {
          // Generic error, underlying cause isn't tracked.
          None
      }
  }

  struct Tourniquet {
      state: States,
      runtime: tokio::runtime::Runtime,
  };

  impl Tourniquet {
      pub fn new() -> Self {
          Tourniquet {
              state: States::Done(State { users: vec![] }),
              runtime: tokio::runtime::Runtime::new().expect("cound not create runtime"),
          }
      }
  }

  impl AsyncMachine for Tourniquet {
      type Events = Events;
      type State = State;
      type States = States;
      type Failures = Failures;
      type StateFuture = StateFuture;

      /// transition gets an event and change the internal data relative to the type of event
      /// and the returns the state of the machine
      fn transition(&mut self, event: Self::Events) -> Result<&Self::States, Self::Failures> {
          let state = self.get_state();
          match (state, event) {
              (_, Events::GetUsers) => {
                  let fut = async {
                    let client = Client::new();
                    let mut res = client.get("http://localhost:3333/users").send().await;
                    match res {
                      Ok(res) => {
                          let mut acc = State { users: vec![] };
                          match res.text().await {
                            Ok(result) => {
                              let users: Vec<User> = serde_json::from_str(result.as_str()).expect("could not serialize to struct");
                              acc.users = users;
                              Ok(acc)
                            }
                            Err(e) => Err(Failures::Message(e.to_string())),
                          }
                      }
                      Err(e) => Err(Failures::Message(e.to_string())),
                    }
                  };
                  *self.get_raw_state_mut() = self.get_runtime().block_on(StateFuture::new(Box::new(fut)))?;
                  self.run()
              }
              // (s, e) => {
              //     Err(Failures::Message(format!("Failure on transition for: {:?}, Event: {:?}", s, e)))
              // }
          }
      }

      /// run computes the state of the machine relative to its state
      fn run(&mut self) -> Result<&Self::States, Self::Failures> {
          match self.get_state() {
              States::Loading(c) => {
                  if c.users.len() > 0 {
                      self.state = States::Done(c.clone());
                  }
                  Ok(&self.state)
              }
              _ => Ok(&self.state)
          }
      }

      fn get_runtime(&mut self) -> &mut tokio::runtime::Runtime {
          &mut self.runtime
      }

      fn get_state(&self) -> &Self::States {
          &self.state
      }

      fn get_raw_state(&self) -> &Self::State {
          match &self.state {
              States::Loading(ref c) | States::Done(ref c) => c
          }
      }

      fn get_raw_state_mut(&mut self) -> &mut Self::State {
          match &mut self.state {
              States::Loading(ref mut c) | States::Done(ref mut c) => c
          }
      }
  };       

  let mut tourniquet = Tourniquet::new();

  assert_eq!(tourniquet.get_state(), &States::Done(State { users: vec![] }));

   let mut file = File::open("./tests/__fixtures__/db.json").unwrap();
   let mut data = String::new();
   file.read_to_string(&mut data).unwrap();

   let state1: State = serde_json::from_str(&data).unwrap();

  let entries = [
      (
       Events::GetUsers,
       States::Done(state1)
      )
  ];

  for (event, expected) in entries.iter() {
      assert_eq!(tourniquet.transition(event.clone()).unwrap(), expected);
  }

  Ok(())
}
