// Minimal Qt bootstrap. Lives in C++ only because KDE's Breeze style for Qt Quick
// (org.kde.desktop) needs a QApplication, which cxx-qt-lib does not wrap.
// LOOTINGGLASS_LAYER_SHELL (set by build.rs on Linux) enables the Wayland layer-shell overlay.
// LOOTINGGLASS_QT_TRAY (set on Windows) adds a QSystemTrayIcon; Linux uses an SNI tray from Rust.
#ifdef LOOTINGGLASS_LAYER_SHELL
#include <LayerShellQt/Window>
#endif
#include <QApplication>
#include <QCursor>
#include <QScreen>
#ifdef LOOTINGGLASS_QT_TRAY
#include <atomic>
#include <QActionGroup>
#include <QMenu>
#include <QSystemTrayIcon>
#endif
#include <QIcon>
#include <QMargins>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QTimer>
#include <QWindow>

#ifdef LOOTINGGLASS_QT_TRAY
// Implemented in Rust (src/platform/windows.rs).
extern "C" void lootingglass_tray_action(int action);
extern "C" int lootingglass_tray_game();
extern "C" int lootingglass_tray_update(char *buf, size_t len);

// Tray actions understood by lootingglass_tray_action.
enum TrayAction { OpenManual = 0, OpenSettings = 1, Quit = 2, OpenAbout = 3, CheckUpdates = 4, GamePoe1 = 10, GamePoe2 = 11 };

// Set once the tray icon is showing; read from Rust's threads by lootingglass_tray_notify.
static std::atomic<QSystemTrayIcon *> trayIcon = nullptr;
// TrayAction of the latest message, or -1. messageClicked doesn't say which message was
// clicked, so an older one still on screen gets the latest one's action. Qt thread only.
static int messageAction = -1;

// Called from Rust (any thread): show a message from the tray icon, a toast on Windows 10/11.
// Returns false when there's no system tray to show it from.
extern "C" bool lootingglass_tray_notify(const char *title, const char *body, int action)
{
    QSystemTrayIcon *tray = trayIcon.load();
    if (!tray)
        return false;
    const QString t = QString::fromUtf8(title), b = QString::fromUtf8(body);
    QMetaObject::invokeMethod(tray, [tray, t, b, action] {
        messageAction = action;
        tray->showMessage(t, b, tray->icon());
    }, Qt::QueuedConnection);
    return true;
}

static void setupTray(QApplication &app)
{
    auto *tray = new QSystemTrayIcon(app.windowIcon(), &app);
    tray->setToolTip(QStringLiteral("Looting Glass"));
    auto *menu = new QMenu();
    // "Update to X.Y.Z…", shown once the update check found a newer release.
    auto *update = menu->addAction(QString());
    auto *updateSep = menu->addSeparator();
    QObject::connect(update, &QAction::triggered, [] { lootingglass_tray_action(OpenAbout); });
    auto *games = new QActionGroup(menu);
    auto *poe1 = menu->addAction(QStringLiteral("Path of Exile"));
    auto *poe2 = menu->addAction(QStringLiteral("Path of Exile 2"));
    for (auto *a : {poe1, poe2}) {
        a->setCheckable(true);
        games->addAction(a);
    }
    QObject::connect(poe1, &QAction::triggered, [] { lootingglass_tray_action(GamePoe1); });
    QObject::connect(poe2, &QAction::triggered, [] { lootingglass_tray_action(GamePoe2); });
    menu->addSeparator();
    QObject::connect(menu->addAction(QStringLiteral("Manual check…")), &QAction::triggered,
                     [] { lootingglass_tray_action(OpenManual); });
    QObject::connect(menu->addAction(QStringLiteral("Settings…")), &QAction::triggered,
                     [] { lootingglass_tray_action(OpenSettings); });
    QObject::connect(menu->addAction(QStringLiteral("Check for updates…")), &QAction::triggered,
                     [] { lootingglass_tray_action(CheckUpdates); });
    menu->addSeparator();
    QObject::connect(menu->addAction(QStringLiteral("Quit")), &QAction::triggered,
                     [] { lootingglass_tray_action(Quit); });
    // The game can change from a hotkey or the settings; refresh the radio items on open.
    QObject::connect(menu, &QMenu::aboutToShow, [poe1, poe2, update, updateSep] {
        (lootingglass_tray_game() == 2 ? poe2 : poe1)->setChecked(true);
        char version[64];
        const bool has = lootingglass_tray_update(version, sizeof version) != 0;
        if (has)
            update->setText(QStringLiteral("Update to %1…").arg(QString::fromUtf8(version)));
        update->setVisible(has);
        updateSep->setVisible(has);
    });
    tray->setContextMenu(menu);
    QObject::connect(tray, &QSystemTrayIcon::activated, [](QSystemTrayIcon::ActivationReason r) {
        if (r == QSystemTrayIcon::Trigger)
            lootingglass_tray_action(OpenManual);
    });
    QObject::connect(tray, &QSystemTrayIcon::messageClicked, [] {
        if (messageAction >= 0)
            lootingglass_tray_action(messageAction);
    });
    tray->show();
    if (QSystemTrayIcon::isSystemTrayAvailable())
        trayIcon = tray;
    QObject::connect(tray, &QObject::destroyed, [] { trayIcon = nullptr; });
}
#endif

// Without KDE's Breeze, use Qt Quick's Fusion style: unlike the native Windows controls it
// draws everything from the palette (the system's, or the theme's), so the overlay follows it
// consistently. Only the Qt Quick style changes: switching the widget
// style too would swap the app palette for Fusion's built-in light one.
static void useFusion()
{
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(QStringLiteral("Fusion"));
}

// breeze: use KDE's Breeze style rather than Fusion (theme::Style, chosen in Rust). A
// QT_QUICK_CONTROLS_STYLE set by the user is left in place either way.
extern "C" int lootingglass_qt_main(bool breeze)
{
    static int argc = 1;
    static char arg0[] = "lootingglass";
    static char *argv[] = {arg0, nullptr};

#ifdef LOOTINGGLASS_LAYER_SHELL
    // Layer-shell (overlay, no decorations, always on top) needs native Wayland. Override a
    // session-wide QT_QPA_PLATFORM=xcb for this process only, then restore it for children.
    const QByteArray prevQpa = qgetenv("QT_QPA_PLATFORM");
    const bool forceWayland = !qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY")
        && qEnvironmentVariableIsEmpty("LOOTINGGLASS_ALLOW_X11");
    if (forceWayland)
        qputenv("QT_QPA_PLATFORM", "wayland");

    QApplication app(argc, argv);

    if (forceWayland) {
        if (prevQpa.isNull())
            qunsetenv("QT_QPA_PLATFORM");
        else
            qputenv("QT_QPA_PLATFORM", prevQpa);
    }
#else
    QApplication app(argc, argv);
#endif
    app.setApplicationName(QStringLiteral("Looting Glass"));
    app.setDesktopFileName(QStringLiteral("dev.lootingglass.LootingGlass"));
    app.setWindowIcon(QIcon::fromTheme(QStringLiteral("dev.lootingglass.LootingGlass"),
                                       QIcon(QStringLiteral(":/lootingglass/icons/lootingglass.svg"))));
    app.setQuitOnLastWindowClosed(false);
    if (!breeze)
        useFusion();
    else if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
#ifdef LOOTINGGLASS_QT_TRAY
    setupTray(app);
#endif

    QQmlApplicationEngine engine;
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/dev/lootingglass/qml/Main.qml")));
    if (engine.rootObjects().isEmpty())
        return 1;

#ifdef LOOTINGGLASS_LAYER_SHELL
    // Turn the overlay into a layer-shell surface. This must happen before the platform
    // window exists, otherwise it becomes a normal decorated toplevel.
    if (auto *w = qobject_cast<QWindow *>(engine.rootObjects().first())) {
        if (w->handle())
            w->destroy();
        auto *ls = LayerShellQt::Window::get(w);
        ls->setScope(QStringLiteral("lootingglass-overlay"));
        ls->setLayer(LayerShellQt::Window::LayerOverlay);
        ls->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityOnDemand);
        ls->setActivateOnShow(true);
        // Open on the active screen (the game's screen when the hotkey is pressed).
        // The surface is recreated on every show, so this is re-evaluated each time.
        ls->setWantsToBeOnActiveScreen(true);
        // Anchors and margins come from config (QML properties); reapply on every show.
        auto apply = [w, ls] {
            const int top = w->property("marginTop").toInt();
            const int side = w->property("marginSide").toInt();
            const bool left = w->property("side").toInt() == 0;
            const LayerShellQt::Window::Anchors anchors(
                LayerShellQt::Window::AnchorTop
                | (left ? LayerShellQt::Window::AnchorLeft : LayerShellQt::Window::AnchorRight));
            const QMargins margins(side, top, side, side);
            if (ls->anchors() != anchors)
                ls->setAnchors(anchors);
            if (ls->margins() != margins)
                ls->setMargins(margins);
        };
        apply();
        QObject::connect(w, &QWindow::visibleChanged, w, apply);
        // QML-declared properties can't be connected to a lambda from here, so poll cheaply
        // while visible to pick up drag-to-move / side changes.
        auto *timer = new QTimer(w);
        timer->setInterval(16);
        QObject::connect(timer, &QTimer::timeout, w, [w, ls, apply] {
            if (!w->isVisible())
                return;
            const QMargins before = ls->margins();
            const auto anchorsBefore = ls->anchors();
            apply();
            if (ls->margins() != before || ls->anchors() != anchorsBefore)
                w->requestUpdate();
        });
        timer->start();
    }
#else
    // Other platforms: frameless always-on-top tool window, placed like the layer-shell
    // surface: on the screen under the cursor (the game's), top margin, left or right side.
    if (auto *w = qobject_cast<QWindow *>(engine.rootObjects().first())) {
        w->setFlags(w->flags() | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool);
        auto place = [w](bool pickScreen) {
            if (pickScreen) {
                if (QScreen *s = QGuiApplication::screenAt(QCursor::pos()))
                    w->setScreen(s);
            }
            const QRect g = w->screen()->geometry();
            const int top = w->property("marginTop").toInt();
            const int side = w->property("marginSide").toInt();
            const bool left = w->property("side").toInt() == 0;
            const QPoint pos(left ? g.x() + side : g.x() + g.width() - w->width() - side, g.y() + top);
            if (w->position() != pos)
                w->setPosition(pos);
        };
        QObject::connect(w, &QWindow::visibleChanged, w, [w, place](bool visible) {
            if (visible) {
                place(true);
                w->requestActivate();
            }
        });
        // Follow drag-to-move / resize / side changes while visible (see the Linux branch).
        auto *timer = new QTimer(w);
        timer->setInterval(16);
        QObject::connect(timer, &QTimer::timeout, w, [w, place] {
            if (w->isVisible())
                place(false);
        });
        timer->start();
    }
#endif
    return app.exec();
}
