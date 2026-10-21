import QtQuick
import QtQuick.Window
import org.kde.taskmanager as TaskManager

Item {
    id: coverage

    property bool covered: false

    function covering(row) {
        const value = role => tasks.data(tasks.index(row, 0), role);
        const roles = TaskManager.AbstractTasksModel;
        return !value(roles.IsMinimized) && (value(roles.IsMaximized) || value(roles.IsFullScreen));
    }

    function update() {
        covered = Array.from({ length: tasks.count }, (_, row) => covering(row)).some(Boolean);
    }

    TaskManager.VirtualDesktopInfo {
        id: desktops
    }

    TaskManager.ActivityInfo {
        id: activities
    }

    TaskManager.TasksModel {
        id: tasks

        groupMode: TaskManager.TasksModel.GroupDisabled
        filterByScreen: true
        filterByVirtualDesktop: true
        filterByActivity: true
        screenGeometry: Qt.rect(Screen.virtualX, Screen.virtualY, Screen.width, Screen.height)
        virtualDesktop: desktops.currentDesktop
        activity: activities.currentActivity
        onDataChanged: coverage.update()
        onCountChanged: coverage.update()
    }
}
