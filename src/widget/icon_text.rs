use glib::{wrapper, Object};
use gtk::{glib, Widget};

mod imp {
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::ObjectSubclass;
    use glib::{derived_properties, object_subclass, GString, Properties};
    use gtk::prelude::{BoxExt, ObjectExt, WidgetExt};
    use gtk::subclass::prelude::{DerivedObjectProperties, ObjectSubclassExt};
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{glib, AccessibleRole, Align, BinLayout, Box, Image, Label, Orientation, Widget};

    use crate::window::SPACING;

    #[derive(Properties)]
    #[properties(wrapper_type = super::IconText)]
    pub struct IconText {
        hbox: Box,
        #[property(name = "icon-name", get = Self::icon_name, set = Self::set_icon_name, type = GString)]
        image: Image,
        #[property(get = Self::label, set = Self::set_label, type = GString)]
        label: Label,
    }

    impl Default for IconText {
        fn default() -> Self {
            Self {
                hbox: Box::builder()
                    .orientation(Orientation::Horizontal)
                    .halign(Align::Center)
                    .build(),
                image: Image::builder()
                    .icon_name("image-missing")
                    .valign(Align::Center)
                    .hexpand(true)
                    .build(),
                label: Label::builder().hexpand(true).visible(false).use_underline(true).build(),
            }
        }
    }

    #[object_subclass]
    impl ObjectSubclass for IconText {
        type ParentType = Widget;
        type Type = super::IconText;

        const NAME: &'static str = "IconText";

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<BinLayout>();
            klass.set_accessible_role(AccessibleRole::Group);
        }
    }

    #[derived_properties]
    impl ObjectImpl for IconText {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_hexpand(false);
            self.label.set_mnemonic_widget(obj.parent().as_ref());
            self.hbox.append(&self.image);
            self.hbox.append(&self.label);
            self.hbox.set_parent(&*obj);
        }

        fn dispose(&self) { self.hbox.unparent(); }
    }

    impl WidgetImpl for IconText {}

    impl IconText {
        fn label(&self) -> GString { self.label.text() }

        fn set_label(&self, text: &str) {
            self.label.set_label(text);
            self.label.set_visible(true);
            self.image.set_hexpand(false);
            self.hbox.set_spacing(SPACING.into());
        }

        fn icon_name(&self) -> GString { self.image.icon_name().unwrap_or_default() }

        fn set_icon_name(&self, icon_name: &str) { self.image.set_icon_name(Some(icon_name)) }
    }
}

wrapper! {
    pub struct IconText(ObjectSubclass<imp::IconText>) @extends Widget;
}

impl IconText {
    pub fn new() -> Self { Object::new() }
}
