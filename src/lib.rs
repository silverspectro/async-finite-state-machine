// https://github.com/rust-lang/rust/issues/54883
#![feature(or_patterns)]

use std::error::Error;

pub trait Machine {
    type Events;
    type State;
    type States;
    type Failures;

    fn transition(&mut self, event: Self::Events) -> Result<&Self::States, Self::Failures>;
    fn run(&mut self) -> Result<&Self::States, Self::Failures>;

    fn get_state(&self) -> &Self::States;
    fn get_raw_state_mut(&mut self) -> &mut Self::State;
    fn get_raw_state(&self) -> &Self::State;
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use super::Machine;

    #[test]
    fn basic_implementation_and_initital_state() -> Result<(), Box<dyn Error>> {
        #[derive(Debug, Clone, PartialEq)]
        struct State {
            coins: Vec<f32>,
            button_pressed: bool,
        };
        #[derive(Debug, Clone, PartialEq)]
        enum Events {
            InsertCoin(f32),
            PressButton(bool),
            Open,
        };
        #[derive(Debug, Clone, PartialEq)]
        enum States {
            Unlocked(State),
            Locked(State),
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
        };

        impl Tourniquet {
            pub fn new() -> Self {
                Tourniquet {
                    state: States::Locked(State { coins: vec![], button_pressed: false })
                }
            }
        }

        impl Machine for Tourniquet {
            type Events = Events;
            type State = State;
            type States = States;
            type Failures = Failures;

            /// transition gets an event and change the internal data relative to the type of event
            /// and the returns the state of the machine
            fn transition(&mut self, event: Self::Events) -> Result<&Self::States, Self::Failures> {
                let state = self.get_state();
                match (state, event) {
                    (_, Events::InsertCoin(value)) => {
                        let new_coins = [&self.get_raw_state().coins[..], &[value]].concat();
                        self.get_raw_state_mut().coins = new_coins;
                        self.run()
                    }
                    (_, Events::PressButton(value)) => {
                        self.get_raw_state_mut().button_pressed = value;
                        self.run()
                    }
                    (States::Unlocked(_c), Events::Open) => {
                        self.get_raw_state_mut().button_pressed = false;
                        self.state = States::Locked(self.get_raw_state().clone());
                        self.run()
                    }
                    (States::Locked(_c), Events::Open) => {
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
                    States::Locked(c) => {
                        if (c.coins.iter().fold(0.0, |acc, v| acc + v) % 10.0) == 0.0 && c.button_pressed {
                            self.state = States::Unlocked(c.clone());
                        }
                        Ok(&self.state)
                    }
                    _ => Ok(&self.state)
                }
            }

            fn get_state(&self) -> &Self::States {
                &self.state
            }

            fn get_raw_state(&self) -> &Self::State {
                match &self.state {
                    States::Unlocked(ref c) | States::Locked(ref c) => c
                }
            }

            fn get_raw_state_mut(&mut self) -> &mut Self::State {
                match &mut self.state {
                    States::Unlocked(ref mut c) | States::Locked(ref mut c) => c
                }
            }
        };

        let mut tourniquet = Tourniquet::new();

        assert_eq!(tourniquet.get_state(), &States::Locked(State { coins: vec![], button_pressed: false }));

        let entries = [
            (Events::InsertCoin(5.0), States::Locked(State { coins: vec![5.0], button_pressed: false })),
            (Events::InsertCoin(5.0), States::Locked(State { coins: vec![5.0, 5.0], button_pressed: false })),
            (Events::PressButton(false), States::Locked(State { coins: vec![5.0, 5.0], button_pressed: false })),
            (Events::PressButton(true), States::Unlocked(State { coins: vec![5.0, 5.0], button_pressed: true })),
            (Events::PressButton(true), States::Unlocked(State { coins: vec![5.0, 5.0], button_pressed: true })),
            (Events::Open, States::Locked(State { coins: vec![5.0, 5.0], button_pressed: false })),
            (Events::InsertCoin(1.0), States::Locked(State { coins: vec![5.0, 5.0, 1.0], button_pressed: false })),
        ];

        for (event, expected) in entries.iter() {
            assert_eq!(tourniquet.transition(event.clone())?, expected);
        }

        Ok(())
    }
}
