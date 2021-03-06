use analysis::imports::Imports;
use analysis::namespaces;
use codegen::general::{self, cfg_deprecated, version_condition, version_condition_string};
use config::gobjects::GObject;
use env::Env;
use file_saver;
use library::*;
use std::io::prelude::*;
use std::io::Result;
use std::path::Path;
use traits::*;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let path = root_path.join("flags.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        let configs: Vec<&GObject> = env.config
            .objects
            .values()
            .filter(|c| {
                c.status.need_generate()
                    && c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
            })
            .collect();

        let mut imports = Imports::new(&env.library);
        imports.add("ffi", None);
        imports.add("glib::translate::*", None);

        for config in &configs {
            if let Type::Bitfield(ref flags) = *env.library.type_(config.type_id.unwrap()) {
                if flags.glib_get_type.is_some() {
                    imports.add("glib::Type", None);
                    imports.add("glib::StaticType", None);
                    imports.add("glib::value::Value", None);
                    imports.add("glib::value::SetValue", None);
                    imports.add("glib::value::FromValue", None);
                    imports.add("glib::value::FromValueOptional", None);
                    imports.add("gobject_ffi", None);
                    break;
                }
            }
        }

        try!(general::start_comments(w, &env.config));
        try!(general::uses(w, env, &imports));
        try!(writeln!(w, ""));

        let mut first = true;
        for config in &configs {
            if let Type::Bitfield(ref flags) = *env.library.type_(config.type_id.unwrap()) {
                if first {
                    mod_rs.push("\nmod flags;".into());
                    first = false;
                }
                if let Some(cfg) = version_condition_string(env, flags.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!("pub use self::flags::{};", flags.name));
                try!(generate_flags(env, w, flags, config));
            }
        }

        Ok(())
    });
}

fn generate_flags(env: &Env, w: &mut Write, flags: &Bitfield, config: &GObject) -> Result<()> {
    try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
    try!(version_condition(w, env, flags.version, false, 0));
    try!(writeln!(w, "bitflags! {{"));
    if config.must_use {
        try!(writeln!(
            w,
            "    #[must_use]"
        ));
    }
    try!(writeln!(w, "    pub struct {}: u32 {{", flags.name));
    for member in &flags.members {
        let member_config = config.members.matched(&member.name);
        let ignore = member_config.iter().any(|m| m.ignore);
        if ignore {
            continue;
        }

        let name = member.name.to_uppercase();
        let val: i64 = member.value.parse().unwrap();
        let deprecated_version = member_config.iter().filter_map(|m| m.deprecated_version).next();
        let version = member_config.iter().filter_map(|m| m.version).next();
        try!(cfg_deprecated(w, env, deprecated_version, false, 2));
        try!(version_condition(w, env, version, false, 2));
        try!(writeln!(w, "\t\tconst {} = {};", name, val as u32));
    }

    try!(writeln!(
        w,
        "{}",
        "    }
}
"
    ));

    try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
    try!(version_condition(w, env, flags.version, false, 0));
    try!(writeln!(
        w,
        "#[doc(hidden)]
impl ToGlib for {name} {{
    type GlibType = ffi::{ffi_name};

    fn to_glib(&self) -> ffi::{ffi_name} {{
        ffi::{ffi_name}::from_bits_truncate(self.bits())
    }}
}}
",
        name = flags.name,
        ffi_name = flags.c_type
    ));

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
    try!(version_condition(w, env, flags.version, false, 0));
    try!(writeln!(
        w,
        "#[doc(hidden)]
impl FromGlib<ffi::{ffi_name}> for {name} {{
    fn from_glib(value: ffi::{ffi_name}) -> {name} {{
        {assert}{name}::from_bits_truncate(value.bits())
    }}
}}
",
        name = flags.name,
        ffi_name = flags.c_type,
        assert = assert
    ));

    if let Some(ref get_type) = flags.glib_get_type {
        try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
        try!(version_condition(w, env, flags.version, false, 0));
        try!(writeln!(
            w,
            "impl StaticType for {name} {{
    fn static_type() -> Type {{
        unsafe {{ from_glib(ffi::{get_type}()) }}
    }}
}}",
            name = flags.name,
            get_type = get_type
        ));
        try!(writeln!(w, ""));

        try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
        try!(version_condition(w, env, flags.version, false, 0));
        try!(writeln!(
            w,
            "impl<'a> FromValueOptional<'a> for {name} {{
    unsafe fn from_value_optional(value: &Value) -> Option<Self> {{
        Some(FromValue::from_value(value))
    }}
}}",
            name = flags.name,
        ));
        try!(writeln!(w, ""));

        try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
        try!(version_condition(w, env, flags.version, false, 0));
        try!(writeln!(
            w,
            "impl<'a> FromValue<'a> for {name} {{
    unsafe fn from_value(value: &Value) -> Self {{
        from_glib(ffi::{ffi_name}::from_bits_truncate(gobject_ffi::g_value_get_flags(value.to_glib_none().0)))
    }}
}}",
            name = flags.name,
            ffi_name = flags.c_type,
        ));
        try!(writeln!(w, ""));

        try!(cfg_deprecated(w, env, flags.deprecated_version, false, 0));
        try!(version_condition(w, env, flags.version, false, 0));
        try!(writeln!(
            w,
            "impl SetValue for {name} {{
    unsafe fn set_value(value: &mut Value, this: &Self) {{
        gobject_ffi::g_value_set_flags(value.to_glib_none_mut().0, this.to_glib().bits())
    }}
}}",
            name = flags.name,
        ));

        try!(writeln!(w, ""));
    }

    Ok(())
}
