pub struct PmindBroker<T: PmindBackend> {
    workers: Vec<tokio::task::JoinHandle<T>>,
}

pub trait PmindBackend {}
