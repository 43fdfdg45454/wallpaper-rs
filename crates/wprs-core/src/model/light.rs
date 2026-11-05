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
        #[serde(default)]
        pub encoding: String,
        #[serde(default)]
        pub max_cll: Option<u32>,
        #[serde(default)]
        pub max_fall: Option<u32>,
    }
}
