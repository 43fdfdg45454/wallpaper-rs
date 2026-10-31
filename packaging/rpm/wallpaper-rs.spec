%global debug_package %{nil}

Name:           wallpaper-rs
Version:        %{wprs_core}
Release:        %{wprs_release}%{?dist}
Summary:        Animated and static wallpapers for KDE Plasma
License:        GPL-3.0-or-later
URL:            https://github.com/43fdfdg45454/wallpaper-rs
Source0:        wallpaper-rs-%{wprs_version}-x86_64-linux.tar.gz
ExclusiveArch:  x86_64
BuildRequires:  systemd-rpm-macros
Requires:       pipewire
Requires:       kpipewire
Requires:       kf6-kwindowsystem
Requires:       plasma-workspace
Requires:       gstreamer1 >= 1.24
Requires:       gstreamer1-plugins-base >= 1.24
Requires:       gstreamer1-plugins-good >= 1.24
Requires:       pipewire-gstreamer
Requires:       kf6-kirigami
Recommends:     wpewebkit >= 2.54
Requires:       kf6-qqc2-desktop-style
Recommends:     %{name}-examples

%description
Images and looping videos as wallpapers, with transitions, one wallpaper per screen and
automatic pausing. A daemon renders each screen on the GPU and KDE Plasma shows it through a
wallpaper plugin; the wallpaper-rs app, wprs-ctl and the system tray icon manage it.

%package examples
Summary:        Example scenes for wallpaper-rs
License:        CC0-1.0
BuildArch:      noarch
Requires:       %{name} = %{version}-%{release}

%description examples
Small scenes made for wallpaper-rs and dedicated to the public domain: layers and blend modes,
particles, effects, a scene that follows the sound, one that follows the pointer and a scripted
one. They appear in the library, can be removed from it, and leave it with this package.

%prep
%autosetup -n wallpaper-rs-%{wprs_version}-x86_64-linux

%install
install -Dm755 -t %{buildroot}%{_bindir} bin/wprsd bin/wprs-ctl bin/wprs-app bin/wprs-web
mkdir -p %{buildroot}%{_userunitdir} %{buildroot}%{_datadir}/dbus-1/services %{buildroot}%{_datadir}/plasma/wallpapers %{buildroot}%{_datadir}/plasma/plasmoids %{buildroot}%{_datadir}/applications
sed 's|@BINDIR@|%{_bindir}|g' packaging/systemd/wprsd.service > %{buildroot}%{_userunitdir}/wprsd.service
sed 's|@BINDIR@|%{_bindir}|g' packaging/dbus/io.github.wallpaper_rs.Daemon.service > %{buildroot}%{_datadir}/dbus-1/services/io.github.wallpaper_rs.Daemon.service
sed 's|@BINDIR@|%{_bindir}|g' packaging/dbus/io.github.wallpaper_rs.service > %{buildroot}%{_datadir}/dbus-1/services/io.github.wallpaper_rs.service
sed 's|@BINDIR@|%{_bindir}|g' packaging/desktop/io.github.wallpaper_rs.desktop > %{buildroot}%{_datadir}/applications/io.github.wallpaper_rs.desktop
install -Dm644 packaging/icons/io.github.wallpaper_rs.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.wallpaper_rs.svg
cp -r plasma-wallpaper %{buildroot}%{_datadir}/plasma/wallpapers/io.github.wallpaper_rs
cp -r plasma-tray %{buildroot}%{_datadir}/plasma/plasmoids/io.github.wallpaper_rs.tray
mkdir -p %{buildroot}%{_datadir}/wallpaper-rs
cp -r kwin-pointer %{buildroot}%{_datadir}/wallpaper-rs/kwin-pointer
install -Dm644 -t %{buildroot}%{_datadir}/wallpaper-rs/examples examples/*.wpak

%files
%license LICENSE
%doc README.md
%{_bindir}/wprsd
%{_bindir}/wprs-ctl
%{_bindir}/wprs-app
%{_bindir}/wprs-web
%{_userunitdir}/wprsd.service
%{_datadir}/dbus-1/services/io.github.wallpaper_rs.Daemon.service
%{_datadir}/dbus-1/services/io.github.wallpaper_rs.service
%{_datadir}/applications/io.github.wallpaper_rs.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.wallpaper_rs.svg
%{_datadir}/plasma/wallpapers/io.github.wallpaper_rs
%{_datadir}/plasma/plasmoids/io.github.wallpaper_rs.tray
%dir %{_datadir}/wallpaper-rs
%{_datadir}/wallpaper-rs/kwin-pointer

%files examples
%license examples/LICENSE
%{_datadir}/wallpaper-rs/examples
