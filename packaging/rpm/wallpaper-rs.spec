%global debug_package %{nil}

Name:           wallpaper-rs
Version:        %{wprs_version}
Release:        1%{?dist}
Summary:        Animated and static wallpapers for KDE Plasma
License:        GPL-3.0-or-later
URL:            https://github.com/43fdfdg45454/wallpaper-rs
Source0:        wallpaper-rs-%{version}-x86_64-linux.tar.gz
ExclusiveArch:  x86_64
BuildRequires:  systemd-rpm-macros
Requires:       pipewire
Requires:       kpipewire
Requires:       kf6-kwindowsystem
Requires:       plasma-workspace
Requires:       gstreamer1-plugins-base
Requires:       gstreamer1-plugins-good
Requires:       kf6-kirigami
Requires:       kf6-qqc2-desktop-style

%description
Images and looping videos as wallpapers, with transitions, one wallpaper per screen and
automatic pausing. A daemon renders each screen on the GPU and KDE Plasma shows it through a
wallpaper plugin; the wallpaper-rs app, wprs-ctl and the system tray icon manage it.

%prep
%autosetup -n wallpaper-rs-%{version}-x86_64-linux

%install
install -Dm755 -t %{buildroot}%{_bindir} bin/wprsd bin/wprs-ctl bin/wprs-app
mkdir -p %{buildroot}%{_userunitdir} %{buildroot}%{_datadir}/dbus-1/services %{buildroot}%{_datadir}/plasma/wallpapers %{buildroot}%{_datadir}/plasma/plasmoids %{buildroot}%{_datadir}/applications
sed 's|@BINDIR@|%{_bindir}|g' packaging/systemd/wprsd.service > %{buildroot}%{_userunitdir}/wprsd.service
sed 's|@BINDIR@|%{_bindir}|g' packaging/dbus/io.github.wallpaper_rs.Daemon.service > %{buildroot}%{_datadir}/dbus-1/services/io.github.wallpaper_rs.Daemon.service
sed 's|@BINDIR@|%{_bindir}|g' packaging/dbus/io.github.wallpaper_rs.service > %{buildroot}%{_datadir}/dbus-1/services/io.github.wallpaper_rs.service
sed 's|@BINDIR@|%{_bindir}|g' packaging/desktop/io.github.wallpaper_rs.desktop > %{buildroot}%{_datadir}/applications/io.github.wallpaper_rs.desktop
install -Dm644 packaging/icons/io.github.wallpaper_rs.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.wallpaper_rs.svg
cp -r plasma-wallpaper %{buildroot}%{_datadir}/plasma/wallpapers/io.github.wallpaper_rs
cp -r plasma-tray %{buildroot}%{_datadir}/plasma/plasmoids/io.github.wallpaper_rs.tray

%files
%license LICENSE
%doc README.md
%{_bindir}/wprsd
%{_bindir}/wprs-ctl
%{_bindir}/wprs-app
%{_userunitdir}/wprsd.service
%{_datadir}/dbus-1/services/io.github.wallpaper_rs.Daemon.service
%{_datadir}/dbus-1/services/io.github.wallpaper_rs.service
%{_datadir}/applications/io.github.wallpaper_rs.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.wallpaper_rs.svg
%{_datadir}/plasma/wallpapers/io.github.wallpaper_rs
%{_datadir}/plasma/plasmoids/io.github.wallpaper_rs.tray
