model! {
    #[derive(Default)]
    pub struct HdrState {
        pub choice: String,
        pub own: Option<String>,
        pub active: bool,
        pub offered: bool,
        pub display: bool,
        pub peak_nits: Option<f64>,
        pub reference_nits: Option<f64>,
        pub reason: String,
    }
}
