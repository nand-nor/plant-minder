use pmindb::PlantDatabaseHandler;

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("Initializing broker");

    let broker_handle = pmind_broker::broker(tokio::time::Duration::from_secs(15), 500)
        .await
        .map_err(|e| {
            log::error!("Error creating broker & handle {e:}");
            e
        })?;

    log::info!("Initializing database");

    let (_db_handle, db_stream_tx, db_state_tx) =
        PlantDatabaseHandler::new_with_db_conn_tasks("sqlite:./test.db").await?;

    // Set up database subscription to all node sensor related events
    broker_handle
        .send(pmind_broker::ClientSubscribe {
            id: 0,
            sensor_readings: db_stream_tx,
            node_status: db_state_tx,
        })
        .await
        .map_err(|e| {
            log::error!("Error sending database subscribe request {e:}");
            e
        })??;

    loop {
        // todo block on broker or SIGINT / SIGQUIT / SIGSTP from stdin
    }

    Ok(())
}
