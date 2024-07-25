use tokio::sync::mpsc::unbounded_channel;

use pmindb::BrokerCoordinator;
use pmindd::{
    event::{
        handle_key_input_events, handle_node_reg_task, handle_sensor_stream_task, Event,
        EventHandler,
    },
    minder::{PlantMinder, PlantMinderResult},
    ui::Tui,
};

#[actix::main]
async fn main() -> PlantMinderResult<()> {
    env_logger::init();

    // TODO pipe logging to file?

    let (stream_tx, stream_rx) = unbounded_channel();
    let (registration_tx, registration_rx) = unbounded_channel();

    let mut broker = BrokerCoordinator::new_no_db_con(
        stream_tx,
        registration_tx,
        tokio::time::Duration::from_secs(15),
    )
    .await?;
    let mut app = PlantMinder::new(500);

    tokio::spawn(async move {
        broker.exec_monitor().await;
    });

    let mut events = EventHandler::new(1, stream_rx, registration_rx);
    let mut tui = Tui::new()?;
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;
        match events.next().await {
            Ok(Event::Tick) => app.tick().await,
            Ok(Event::Key(key_event)) => handle_key_input_events(key_event, &mut app).await,
            Ok(Event::SensorNodeEvent(r)) => {
                handle_sensor_stream_task(&mut app, r).await;
            }
            Ok(Event::NodeRegistration(n)) => {
                handle_node_reg_task(&mut app, n).await;
            }
            Err(e) => {
                log::error!("Error in app event loop {e:}, exiting");
                break;
            }
        }
    }

    tui.exit()?;

    Ok(())
}
