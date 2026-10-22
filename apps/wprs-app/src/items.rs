#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(i32, total)]
        #[qproperty(bool, loading)]
        #[qproperty(QString, error)]
        #[qproperty(i32, size)]
        #[qproperty(i32, selected)]
        #[qproperty(bool, removable)]
        #[qproperty(bool, imported)]
        #[qproperty(bool, favorite)]
        type ItemModel = super::ItemModelRust;

        #[qinvokable]
        fn search(self: Pin<&mut ItemModel>, text: &QString, kind: &QString, favorite: bool, tag: &QString);
        #[qinvokable]
        fn playlist(self: Pin<&mut ItemModel>, uid: &QString);
        #[qinvokable]
        fn visible(self: Pin<&mut ItemModel>, first: i32, last: i32);
        #[qinvokable]
        fn toggle(self: Pin<&mut ItemModel>, row: i32, only: bool);
        #[qinvokable]
        fn selection(self: &ItemModel) -> QString;
        #[qinvokable]
        fn uid(self: &ItemModel, row: i32) -> QString;
        #[cxx_override]
        fn data(self: &ItemModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        fn role_names(self: &ItemModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        fn row_count(self: &ItemModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        fn can_fetch_more(self: &ItemModel, parent: &QModelIndex) -> bool;
        #[cxx_override]
        fn fetch_more(self: Pin<&mut ItemModel>, parent: &QModelIndex);
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[inherit]
        unsafe fn begin_insert_rows(self: Pin<&mut ItemModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        unsafe fn end_insert_rows(self: Pin<&mut ItemModel>);
        #[inherit]
        unsafe fn begin_remove_rows(self: Pin<&mut ItemModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        unsafe fn end_remove_rows(self: Pin<&mut ItemModel>);
        #[inherit]
        unsafe fn begin_move_rows(
            self: Pin<&mut ItemModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
            destination: &QModelIndex,
            row: i32,
        ) -> bool;
        #[inherit]
        unsafe fn end_move_rows(self: Pin<&mut ItemModel>);
        #[inherit]
        unsafe fn begin_reset_model(self: Pin<&mut ItemModel>);
        #[inherit]
        unsafe fn end_reset_model(self: Pin<&mut ItemModel>);
        #[inherit]
        fn index(self: &ItemModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        fn data_changed(
            self: Pin<&mut ItemModel>,
            first: &QModelIndex,
            last: &QModelIndex,
            roles: &QVector_i32,
        );
    }

    impl cxx_qt::Threading for ItemModel {}
    impl cxx_qt::Initialize for ItemModel {}
}

use crate::library::ItemModelRust;
