use adafruit_dht11_sys::pi_2_dht_read;
/*use hal_sensor_dht::SensorType;
use libc::sched_param;
use libc::sched_setscheduler;
use libc::SCHED_FIFO;
use libc::SCHED_OTHER;*/

/*struct MyInterruptCtrl;

impl hal_sensor_dht::InterruptCtrl for MyInterruptCtrl {
    fn enable(&mut self) {
        unsafe {
            let param = sched_param { sched_priority: 32 };
            let result = sched_setscheduler(0, SCHED_FIFO, &param);

            if result != 0 {
                panic!("Error setting priority, you may not have cap_sys_nice capability");
            }
        }
    }
    fn disable(&mut self) {
        unsafe {
            let param = sched_param { sched_priority: 0 };
            let result = sched_setscheduler(0, SCHED_OTHER, &param);

            if result != 0 {
                panic!("Error setting priority, you may not have cap_sys_nice capability");
            }
        }
    }
}*/

struct Measurement {
    temperature: f32,
    humidity: f32,
}

fn dht_read(pin: i32) -> Result<Measurement, i32> {
    let mut temperature = 0.0;
    let mut humidity = 0.0;
    let result = unsafe { pi_2_dht_read(11, pin, &mut humidity, &mut temperature) };
    match result {
        0 => Ok(Measurement {
            temperature,
            humidity,
        }),
        err => Err(err),
    }
}

fn main() {
    loop {
        match dht_read(17) {
            Ok(measure) => println!(
                "Measured: temp={}* humidity={}%",
                measure.temperature, measure.humidity
            ),
            Err(err) => println!("Error: {}", err),
        }
    }
}
