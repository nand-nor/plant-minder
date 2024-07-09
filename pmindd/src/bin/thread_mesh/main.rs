use pmindb::BrokerCoordinator;

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("Initializing broker & starting task loops");
    let mut broker = BrokerCoordinator::new().await?;

    broker.exec_task_loops().await;

    loop {
        // todo block on broker or SIGINT / SIGQUIT / SIGSTP from stdin
    }

    Ok(())
}
