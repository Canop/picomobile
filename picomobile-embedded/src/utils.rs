use {
    embassy_time::Timer,
    panic_halt as _,
};

pub async fn expect<T, E: core::fmt::Debug>(res: Result<T, E>) -> T {
    match res {
        Ok(v) => v,
        Err(e) => {
            log::error!("Fatal error: {:?}", e);
            Timer::after_millis(100).await;
            panic!("Fatal error: {:?}", e);
        }
    }
}
