

pub struct PmindBroker<T: PmindBackend> {
    workers: Vec<tokio::task::JoinHandle<T>>
}

pub trait PmindBackend {}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
