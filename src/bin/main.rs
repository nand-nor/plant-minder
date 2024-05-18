use linux_embedded_hal::I2cdev;
use plant_minder::{soil_sensor::SoilSensor, PlantMinderError};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use xca9548a::{SlaveAddr, Xca9548a};

#[tokio::main]
async fn main() -> Result<(), PlantMinderError> {
    let dev = I2cdev::new("/dev/i2c-1")?;
    let address = SlaveAddr::default();

    let i2c_switch: &'static mut _ = Box::leak(Box::new(Xca9548a::new(dev, address)));
    let parts = i2c_switch.split();

    let sensor_0 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c0, 0x36)));
    let sensor_1 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c1, 0x36)));
    let sensor_2 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c2, 0x36)));
    let sensor_3 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c3, 0x36)));

    loop {
        read_sensor(Arc::clone(&sensor_0), "Plant Sensor 0".to_string()).await?;
        read_sensor(Arc::clone(&sensor_1), "Plant Sensor 1".to_string()).await?;
        read_sensor(Arc::clone(&sensor_2), "Plant Sensor 2".to_string()).await?;
        read_sensor(Arc::clone(&sensor_3), "Plant Sensor 3".to_string()).await?;
        sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

async fn read_sensor(
    sensor: Arc<Mutex<SoilSensor>>,
    title: String,
) -> Result<(), PlantMinderError> {
    if let Err(e) = tokio::spawn(async move {
        let mut guard = sensor.lock().await;
        let temp = guard.temperature().await?;
        let moist = guard.moisture().await?;
        println!("Plant sensor {title} reading: {temp:?} {moist:?}");
        Ok::<(), PlantMinderError>(())
    })
    .await
    {
        // to do log error
    }
    Ok(())
}
