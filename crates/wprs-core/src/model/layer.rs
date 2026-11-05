model! {
    #[derive(Default)]
    pub struct LayerShellState {
        pub namespace: Option<String>,
        pub from: String,
        pub restart: bool,
    }
}
