use gdk;
use std::cell::Cell;

use anyhow::Result;

pub fn gtk() -> Result<()> {
    gtk::init()?;

    let disp = gdk::Display::get_default().ok_or(anyhow::anyhow!("failed to open display"))?;
    let clip = gtk::Clipboard::get_for_display(&disp, &gdk::SELECTION_CLIPBOARD);

    // Do we need to support TEXT, COMPOUND_TEXT and others from
    // gtk_target_list_add_text_targets()?
    let targ = gtk::TargetEntry::new("UTF8_STRING", gtk::TargetFlags::empty(), 0);

    // First request comes immediately as we claim to have a selection, apparently from a system
    // that preserves clipboard value after the main program exit. Naturally we want to ignore
    // that, but it might not be universal (non gnome sesisons may not have that).
    let cnt = Cell::new(0);
    clip.set_with_data(&[targ], move |_clip, sd, _| {
        if cnt.get() == 0 {
            cnt.set(1);
            return;
        }

        sd.set_text("Hello world");
        gtk::main_quit();
    });

    gtk::main();

    Ok(())
}
