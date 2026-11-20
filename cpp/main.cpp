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

// Tray actions understood by lootingglass_tray_action.
enum TrayAction { OpenManual = 0, OpenSettings = 1, Quit = 2, GamePoe1 = 10, GamePoe2 = 11 };

static void setupTray(QApplication &app)
{
    auto *tray = new QSystemTrayIcon(app.windowIcon(), &app);
    tray->setToolTip(QStringLiteral("Looting Glass"));
    auto *menu = new QMenu();
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
    menu->addSeparator();
    QObject::connect(menu->addAction(QStringLiteral("Quit")), &QAction::triggered,
                     [] { lootingglass_tray_action(Quit); });
    // The game can change from a hotkey or the settings; refresh the radio items on open.
    QObject::connect(menu, &QMenu::aboutToShow, [poe1, poe2] {
        (lootingglass_tray_game() == 2 ? poe2 : poe1)->setChecked(true);
    });
    tray->setContextMenu(menu);
    QObject::connect(tray, &QSystemTrayIcon::activated, [](QSystemTrayIcon::ActivationReason r) {
        if (r == QSystemTrayIcon::Trigger)
            lootingglass_tray_action(OpenManual);
    });
    tray->show();
}
#endif

// Without KDE's Breeze, use Qt Quick's Fusion style: unlike the native Windows controls it
// draws everything from the system palette, so the overlay follows the system's light or dark
// mode (and accent colour) consistently. Only the Qt Quick style changes: switching the widget
// style too would swap the app palette for Fusion's built-in light one.
static void useFusion()
{
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(QStringLiteral("Fusion"));
}

extern "C" int lootingglass_qt_main()
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
#ifdef LOOTINGGLASS_LAYER_SHELL
    // KDE's Breeze style (follows the user's colour scheme). LOOTINGGLASS_FUSION=1 previews the
    // style other platforms get.
    if (!qEnvironmentVariableIsEmpty("LOOTINGGLASS_FUSION"))
        useFusion();
    else if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
#else
    useFusion();
#endif
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
