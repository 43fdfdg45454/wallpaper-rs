import QtQuick
import QtQuick.Window
import org.kde.kwindowsystem
import org.kde.taskmanager as TaskManager

Item {
    id: coverage

    property bool windows: false
    readonly property bool covered: windows && !KWindowSystem.showingDesktop

    function covering(row) {
        const value = role => tasks.data(tasks.index(row, 0), role);
        const roles = TaskManager.AbstractTasksModel;
        const within = (list, ids) => !list?.length || ids.some(id => list.includes(id));
        const desktop = desktops.currentDesktopByScreenName?.(Screen.name) ?? desktops.currentDesktop;
        const here = value(roles.IsOnAllVirtualDesktops) || within(value(roles.VirtualDesktops), [desktop]);
        const active = within(value(roles.Activities), [activities.currentActivity, "00000000-0000-0000-0000-000000000000"]);
        return (value(roles.IsMaximized) || value(roles.IsFullScreen)) && (!value(roles.IsDemandingAttention) || here && active);
    }

    function update() {
        windows = [...Array(tasks.count).keys()].some(row => covering(row));
    }

    Connections {
        target: desktops
        ignoreUnknownSignals: true

        function onCurrentDesktopChanged() {
            coverage.update();
        }

        function onCurrentDesktopForScreenChanged() {
            coverage.update();
        }
    }

    TaskManager.VirtualDesktopInfo {
        id: desktops
    }

    TaskManager.ActivityInfo {
        id: activities

        onCurrentActivityChanged: coverage.update()
    }

    TaskManager.TasksModel {
        id: tasks

        groupMode: TaskManager.TasksModel.GroupDisabled
        filterByScreen: true
        filterByVirtualDesktop: true
        filterByActivity: true
        filterMinimized: true
        filterHidden: true
        screenGeometry: Qt.rect(Screen.virtualX, Screen.virtualY, Screen.width, Screen.height)
        virtualDesktop: desktops.currentDesktop
        activity: activities.currentActivity
        onDataChanged: coverage.update()
        onCountChanged: coverage.update()
        onModelReset: coverage.update()
        Component.onCompleted: {
            if (tasks.filterByCurrentVirtualDesktop === undefined)
                return;
            tasks.filterByVirtualDesktop = false;
            tasks.filterByCurrentVirtualDesktop = true;
        }
    }
}
