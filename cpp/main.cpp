// Minimal Qt bootstrap. Lives in C++ only because KDE's Breeze style for Qt Quick
// (org.kde.desktop) needs a QApplication, which cxx-qt-lib does not wrap.
// LOOTLENS_LAYER_SHELL (set by build.rs on Linux) enables the Wayland layer-shell overlay.
#ifdef LOOTLENS_LAYER_SHELL
#include <LayerShellQt/Window>
#endif
#include <QApplication>
#include <QIcon>
#include <QMargins>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QTimer>
#include <QWindow>

extern "C" int lootlens_qt_main()
{
    static int argc = 1;
    static char arg0[] = "lootlens";
    static char *argv[] = {arg0, nullptr};

#ifdef LOOTLENS_LAYER_SHELL
    // Layer-shell (overlay, no decorations, always on top) needs native Wayland. Override a
    // session-wide QT_QPA_PLATFORM=xcb for this process only, then restore it for children.
    const QByteArray prevQpa = qgetenv("QT_QPA_PLATFORM");
    const bool forceWayland = !qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY")
        && qEnvironmentVariableIsEmpty("LOOTLENS_ALLOW_X11");
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
    app.setApplicationName(QStringLiteral("Loot Lens"));
    app.setDesktopFileName(QStringLiteral("dev.lootlens.LootLens"));
    app.setWindowIcon(QIcon::fromTheme(QStringLiteral("dev.lootlens.LootLens")));
    app.setQuitOnLastWindowClosed(false);
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));

    QQmlApplicationEngine engine;
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/dev/lootlens/qml/Main.qml")));
    if (engine.rootObjects().isEmpty())
        return 1;

#ifdef LOOTLENS_LAYER_SHELL
    // Turn the overlay into a layer-shell surface. This must happen before the platform
    // window exists, otherwise it becomes a normal decorated toplevel.
    if (auto *w = qobject_cast<QWindow *>(engine.rootObjects().first())) {
        if (w->handle())
            w->destroy();
        auto *ls = LayerShellQt::Window::get(w);
        ls->setScope(QStringLiteral("lootlens-overlay"));
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
    // Other platforms: frameless always-on-top tool window. Positioning from marginTop/
    // marginSide/side still needs implementing for the Windows port.
    if (auto *w = qobject_cast<QWindow *>(engine.rootObjects().first()))
        w->setFlags(w->flags() | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool);
#endif
    return app.exec();
}
