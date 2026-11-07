#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, overlay)]
        #[qproperty(QString, outline)]
        #[qproperty(QString, shown)]
        #[qproperty(QString, other)]
        #[qproperty(QString, survey)]
        #[qproperty(QString, stage)]
        #[qproperty(QString, error)]
        #[qproperty(f64, progress)]
        #[qproperty(bool, busy)]
        #[qproperty(u32, built)]
        type Relief = super::ReliefRust;

        #[qinvokable]
        fn open(self: Pin<&mut Relief>, path: &QString);
        #[qinvokable]
        fn look(self: Pin<&mut Relief>, edit: &QString, colour: bool);
        #[qinvokable]
        fn trace(self: Pin<&mut Relief>, edit: &QString, cuts: &QString);
        #[qinvokable]
        fn measure(self: Pin<&mut Relief>, edit: &QString);
        #[qinvokable]
        fn sync(self: Pin<&mut Relief>, edit: &QString);
        #[qinvokable]
        fn view(self: Pin<&mut Relief>, edit: &QString, layer: u32, band: bool);
        #[qinvokable]
        fn compare(self: Pin<&mut Relief>, edit: &QString, layer: u32);
        #[qinvokable]
        fn cancel(self: Pin<&mut Relief>);
        #[qinvokable]
        fn clear(self: Pin<&mut Relief>);
    }

    impl cxx_qt::Threading for Relief {}
}

use cxx_qt_lib::QString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub struct ReliefRust {
    pub overlay: QString,
    pub outline: QString,
    pub shown: QString,
    pub other: QString,
    pub survey: QString,
    pub stage: QString,
    pub error: QString,
    pub progress: f64,
    pub busy: bool,
    pub built: u32,
    pub file: PathBuf,
    pub stop: Arc<AtomicBool>,
    pub waiting: [Option<String>; 3],
    pub working: [bool; 3],
    pub turn: u64,
}
