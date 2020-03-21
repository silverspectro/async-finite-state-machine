extern crate tokio;
extern crate futures;

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

pub trait AsyncMachine {
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
