model! {
    #[derive(Default)]
    pub struct LayerShellState {
        pub namespace: Option<String>,
        pub from: String,
        pub restart: bool,
        #[serde(default)]
        pub ignored: Option<String>,
        #[serde(default)]
        pub ignored_from: Option<String>,
        #[serde(default)]
        pub ignored_empty: bool,
    }
}
