//! Records requests at the platform boundary while the normal runtime routes input.
use std::sync::{Arc, Mutex};
use zgui::{geom::*, platform::*, runtime::Runtime};

#[derive(Debug, Default)]
pub struct State {
    pub moves: usize,
    pub resizes: Vec<ResizeEdge>,
    pub minimized: bool,
    pub maximized: bool,
    pub fullscreen: Option<FullscreenMode>,
}

pub struct RecordingApp {
    pub runtime: Runtime,
    pub state: Arc<Mutex<State>>,
}
impl AppHandler for RecordingApp {
    fn surfaces_available(&mut self, cx: &dyn PlatformCx) {
        self.runtime
            .surfaces_available(&RecordingCx(cx, self.state.clone()));
    }
    fn surface_event(&mut self, cx: &dyn PlatformCx, id: SurfaceId, event: SurfaceEvent) {
        self.runtime.surface_event(cx, id, event);
    }
    fn wake(&mut self, cx: &dyn PlatformCx, reason: WakeReason) {
        self.runtime.wake(cx, reason);
    }
    fn idle(&mut self, cx: &dyn PlatformCx) -> IdlePolicy {
        self.runtime.idle(cx)
    }
    fn deadline_reached(&mut self, cx: &dyn PlatformCx) {
        self.runtime.deadline_reached(cx);
    }
}

struct RecordingCx<'a>(&'a dyn PlatformCx, Arc<Mutex<State>>);
impl PlatformCx for RecordingCx<'_> {
    fn create_surface(
        &self,
        attributes: &SurfaceAttributes,
    ) -> Result<Arc<dyn Surface>, PlatformError> {
        Ok(Arc::new(RecordingSurface(
            self.0.create_surface(attributes)?,
            self.1.clone(),
        )))
    }
    fn destroy_surface(&self, id: SurfaceId) {
        self.0.destroy_surface(id);
    }
    fn surface(&self, id: SurfaceId) -> Option<Arc<dyn Surface>> {
        self.0.surface(id)
    }
    fn surfaces(&self) -> Vec<Arc<dyn Surface>> {
        self.0.surfaces()
    }
    fn monitors(&self) -> Vec<MonitorInfo> {
        self.0.monitors()
    }
    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.0.primary_monitor()
    }
    fn color_scheme(&self) -> Option<ColorScheme> {
        self.0.color_scheme()
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self.0.clipboard()
    }
    fn capabilities(&self) -> &PlatformCapabilities {
        self.0.capabilities()
    }
    fn clock(&self) -> Arc<dyn Clock> {
        self.0.clock()
    }
    fn waker(&self) -> Arc<dyn Waker> {
        self.0.waker()
    }
    fn request_exit(&self) {
        self.0.request_exit();
    }
    fn is_exiting(&self) -> bool {
        self.0.is_exiting()
    }
}

struct RecordingSurface(Arc<dyn Surface>, Arc<Mutex<State>>);
impl Surface for RecordingSurface {
    fn id(&self) -> SurfaceId {
        self.0.id()
    }
    fn size(&self) -> Size<DevicePx, Device> {
        self.0.size()
    }
    fn scale_factor(&self) -> f64 {
        self.0.scale_factor()
    }
    fn request_size(&self, size: Size<CssPx, Css>) -> Option<Size<DevicePx, Device>> {
        self.0.request_size(size)
    }
    fn set_min_size(&self, size: Option<Size<CssPx, Css>>) {
        self.0.set_min_size(size);
    }
    fn set_max_size(&self, size: Option<Size<CssPx, Css>>) {
        self.0.set_max_size(size);
    }
    fn request_redraw(&self) {
        self.0.request_redraw();
    }
    fn pre_present_notify(&self) {
        self.0.pre_present_notify();
    }
    fn set_title(&self, title: &str) {
        self.0.set_title(title);
    }
    fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible);
    }
    fn set_decorations(&self, value: Decorations) {
        self.0.set_decorations(value);
    }
    fn set_resizable(&self, value: bool) {
        self.0.set_resizable(value);
    }
    fn set_maximized(&self, value: bool) {
        self.1.lock().unwrap().maximized = value;
        self.request_redraw();
    }
    fn is_maximized(&self) -> bool {
        self.1.lock().unwrap().maximized
    }
    fn set_minimized(&self, value: bool) {
        self.1.lock().unwrap().minimized = value;
    }
    fn set_fullscreen(&self, value: Option<FullscreenMode>) {
        self.1.lock().unwrap().fullscreen = value;
        self.request_redraw();
    }
    fn fullscreen(&self) -> Option<FullscreenMode> {
        self.1.lock().unwrap().fullscreen
    }
    fn begin_move_drag(&self) -> Result<(), Unsupported> {
        self.1.lock().unwrap().moves += 1;
        Ok(())
    }
    fn begin_resize_drag(&self, edge: ResizeEdge) -> Result<(), Unsupported> {
        self.1.lock().unwrap().resizes.push(edge);
        Ok(())
    }
    fn set_cursor(&self, cursor: CursorStyle) {
        self.0.set_cursor(cursor);
    }
    fn set_text_input(&self, input: Option<TextInput>) {
        self.0.set_text_input(input);
    }
    fn push_a11y_update(&self, build: &mut dyn FnMut() -> accesskit::TreeUpdate) {
        self.0.push_a11y_update(build);
    }
}
