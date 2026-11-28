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
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QRegion>
#include <QTimer>
#include <QWindow>

#ifdef LOOTINGGLASS_QT_TRAY
// Implemented in Rust (src/platform/windows.rs).
extern "C" void lootingglass_tray_action(int action);
extern "C" int lootingglass_tray_game();
extern "C" int lootingglass_tray_update(char *buf, size_t len);

// Tray actions understood by lootingglass_tray_action (TRAY_ACTIONS in windows.rs).
enum TrayAction { NoAction = -1, OpenManual = 0, OpenSettings = 1, Quit = 2, OpenAbout = 3, CheckUpdates = 4, GamePoe1 = 10, GamePoe2 = 11 };

// The tray icon, touched on the Qt thread only.
static QSystemTrayIcon *trayIcon = nullptr;
// Whether it can show messages; read from Rust's threads. Quitting from the app exits the process
// directly; when the event loop ends some other way (Windows signing out), this is cleared on
// aboutToQuit, so messages stop being posted before QApplication goes away.
static std::atomic<bool> trayReady = false;
// TrayAction of the latest message. messageClicked doesn't say which message was clicked, so an
// older one still on screen gets the latest one's action. Qt thread only.
static int messageAction = NoAction;

// Called from Rust (any thread): show a message from the tray icon, a toast on Windows 10/11.
// Returns false when there's no system tray to show it from.
extern "C" bool lootingglass_tray_notify(const char *utf8Title, const char *utf8Body, int action)
{
    if (!trayReady)
        return false;
    QMetaObject::invokeMethod(qApp, [title = QString::fromUtf8(utf8Title), body = QString::fromUtf8(utf8Body), action] {
        messageAction = action;
        trayIcon->showMessage(title, body, trayIcon->icon());
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
        if (messageAction != NoAction)
            lootingglass_tray_action(messageAction);
    });
    tray->show();
    trayIcon = tray;
    trayReady = QSystemTrayIcon::isSystemTrayAvailable();
    QObject::connect(&app, &QCoreApplication::aboutToQuit, [] { trayReady = false; });
}
#endif

// The monitor called `name` (QScreen::name, like "DP-1"), or null when there's none.
static QScreen *screenNamed(const QString &name)
{
    if (name.isEmpty())
        return nullptr;
    for (QScreen *s : QGuiApplication::screens()) {
        if (s->name() == name)
            return s;
    }
    return nullptr;
}

#ifndef LOOTINGGLASS_LAYER_SHELL
// The overlay's size (QML's overlayWidth/Height): the window is that size here, while the
// layer-shell surface covers its monitor.
static QSize overlaySize(const QWindow *w)
{
    return QSize(w->property("overlayWidth").toInt(), w->property("overlayHeight").toInt());
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
        // The surface covers its whole monitor (over panels) and is transparent. QML draws the
        // overlay's card where its margins put it, and the input region is the card, so clicks
        // beside it reach the game. Moving or resizing the overlay then only redraws the card:
        // moving the surface itself would need the compositor, which applies it a frame or more
        // after the pointer events that asked for it, so a drag would fight its own moves.
        using LayerShellQt::Window;
        ls->setAnchors(Window::Anchors(Window::AnchorTop | Window::AnchorBottom | Window::AnchorLeft
                                       | Window::AnchorRight));
        ls->setExclusiveZone(-1);
        w->setProperty("coversMonitor", true);
        // The monitor and input region come from QML properties; reapply on every show. The
        // surface is recreated on every show, so its monitor is chosen each time.
        auto apply = [w, ls] {
            if (QScreen *s = screenNamed(w->property("overlayScreen").toString())) {
                if (ls->screen() != s)
                    ls->setScreen(s);
            } else if (!ls->wantsToBeOnActiveScreen()) {
                // The active screen: the game's when the hotkey is pressed.
                ls->setWantsToBeOnActiveScreen(true);
            }
            const QRegion card(w->property("cardRect").toRect());
            if (w->mask() != card)
                w->setMask(card);
        };
        apply();
        QObject::connect(w, &QWindow::visibleChanged, w, apply);
        // QML-declared properties can't be connected to a lambda from here, so poll cheaply
        // while visible to pick up moves, resizes and monitor changes.
        auto *timer = new QTimer(w);
        timer->setInterval(16);
        QObject::connect(timer, &QTimer::timeout, w, [w, ls, apply] {
            if (!w->isVisible())
                return;
            QScreen *screenBefore = ls->screen();
            apply();
            // A layer surface can't change monitors: moved to another one, show it again there.
            if (ls->screen() != screenBefore) {
                w->hide();
                w->show();
            }
        });
        timer->start();
    }
#else
    // Other platforms: frameless always-on-top tool window the size of the overlay, on the
    // monitor it was moved to or else the one under the cursor (the game's), where QML puts it.
    if (auto *w = qobject_cast<QWindow *>(engine.rootObjects().first())) {
        w->setFlags(w->flags() | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool);
        auto place = [w](bool pickScreen) {
            QScreen *s = screenNamed(w->property("overlayScreen").toString());
            if (!s && pickScreen)
                s = QGuiApplication::screenAt(QCursor::pos());
            if (s && w->screen() != s)
                w->setScreen(s);
            const QRect g = w->screen()->geometry();
            const QRect placed(g.topLeft() + QPoint(w->property("cardX").toInt(), w->property("marginTop").toInt()),
                               overlaySize(w));
            if (w->geometry() != placed)
                w->setGeometry(placed);
        };
        QObject::connect(w, &QWindow::visibleChanged, w, [w, place](bool visible) {
            if (visible) {
                place(true);
                w->requestActivate();
            }
        });
        // Follow moves, resizes and side or monitor changes while visible.
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
