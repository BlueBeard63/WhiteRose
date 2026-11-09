//! linux-dmabuf: lets clients hand over GPU buffers instead of copying
//! pixels through shared memory. GTK4 and Qt need it for GPU rendering.

use smithay::{
    backend::allocator::dmabuf::Dmabuf,
    delegate_dmabuf,
    wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
};

use crate::state::WhiteRose;

impl DmabufHandler for WhiteRose {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        // Only the backend owns a renderer; without one (headless) every import fails.
        let ok = self.dmabuf_importer.as_mut().is_some_and(|import| import(&dmabuf));
        if ok {
            let _ = notifier.successful::<WhiteRose>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(WhiteRose);
