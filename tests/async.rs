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

use std::thread;
use std::sync::mpsc;
use std::time::Duration;

use {
    std::{
        future::Future,
        task::{Poll},
    },
};

use std::sync::mpsc::*;

#[test]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> { 
  // StatesFuture needs that ? :
  // https://rust-lang.github.io/async-book/02_execution/03_wakeups.html
  // to handle the assignation of the state and the Loading ?
  // @TODO
  // Implement Executor for AsyncMachine ?
  // https://rust-lang.github.io/async-book/02_execution/04_executor.html
  struct StatesFuture {
    inner: Pin<Box<dyn Future<Output = Result<States, Failures>> + Send>>,
  }
  impl StatesFuture {
      fn new(fut: Box<dyn Future<Output = Result<States, Failures>> + Send>) -> Self {
          Self { inner: fut.into() }
      }
  }

  impl fmt::Debug for StatesFuture {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          f.pad("Future<States>")
      }
  }

  impl Future for StatesFuture {
      type Output = Result<States, Failures>;

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
      eye_color: String,
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

  #[derive(Debug)]
  struct Tourniquet {
      state: States,
      future_pool: Vec<StatesFuture>,
      channel: (Vec<Sender<States>>, Receiver<States>),
  };

  impl Tourniquet {
      pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let channel = (vec![sender], receiver);
          Tourniquet {
              state: States::Done(State { users: vec![] }),
              future_pool: vec![],
              channel,
          }
      }
  }

  impl<'a> AsyncMachine for Tourniquet {
      type Events = Events;
      type State = State;
      type States = States;
      type Failures = Failures;

      /// transition gets an event and change the internal data relative to the type of event
      /// and the returns the state of the machine
      fn transition(&mut self, event: Self::Events) -> Result<&Self::States, Self::Failures> {
          match (self.get_state(), event) {
              (_, Events::GetUsers) => {
                self.state = States::Loading(self.get_raw_state().clone());
                let f = StatesFuture::new(Box::new(async {
                  let client = Client::new();
                  let res = client.get("http://localhost:3333/users").send().await;
                  match res {
                    Ok(res) => {
                        match res.text().await {
                          Ok(result) => {
                            let users: Vec<User> = serde_json::from_str(result.as_str()).expect("could not serialize to struct");
                            Ok(States::Done(State { users }))
                          }
                          Err(e) => Err(Failures::Message(e.to_string())),
                        }
                    }
                    Err(e) => Err(Failures::Message(e.to_string())),
                  }
                }));
                self.future_pool.push(f);
                self.run()
              } 
          }
      }

      /// run computes the state of the machine relative to its state
      fn run(&mut self) -> Result<&Self::States, Self::Failures> {
        while self.future_pool.len() > 0 {
          let f = self.future_pool.pop();
          let sender = self.channel.0[0].clone();
          thread::spawn(move || {
            let mut runtime = tokio::runtime::Runtime::new().unwrap();
            match runtime.block_on(f.unwrap()) {
              Ok(s) => {
                sender.send(s).unwrap();
              },
              Err(e) => println!("{:?}", e),
            };
          });
        };
        match self.channel.1.try_recv() {
          Ok(s) => {
            self.state = s;
          }
          Err(_e) => {
            // messages are empty
            // nothing to do
          }
        };
        match self.get_state() {
            _ => Ok(&self.state)
        }
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
       States::Loading(State { users: vec![] }),
       States::Done(state1.clone())
      ),
      (
       Events::GetUsers,
       States::Loading(state1.clone()),
       States::Done(state1.clone())
      )
  ];

  // execute the request in another thread
  // Pass the state to Loading
  // and test the Done status with a timeout
  // or whatever
  // DONE !
  //
  // seems to work well for the moment
  // certainly there will be some performance problems
  // with complex cases
  for (event, before, after) in entries.iter() {
      assert_eq!(tourniquet.transition(event.clone())?, before);
      thread::sleep(Duration::from_millis(300)); 
      assert_eq!(tourniquet.run()?, after);
      assert_eq!(tourniquet.run()?, after);
  }
  assert_eq!(tourniquet.get_state(), &States::Done(state1.clone()));

  Ok(())
}
