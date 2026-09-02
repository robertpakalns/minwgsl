#![no_std]
extern crate alloc;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
    vec::Vec,
};
use hashbrown::HashMap;
use naga::{
    Expression, Function, Handle, Module, StructMember, Type, TypeInner, UniqueArena,
    back::wgsl as wgsl_back,
    compact::{KeepUnused, compact},
    front::wgsl as wgsl_front,
    valid::{Capabilities, ValidationFlags, Validator},
};
use wasm_bindgen::prelude::wasm_bindgen;

const FIRST_LETTERS: [char; 52] = [
    'A', 'a', 'B', 'b', 'C', 'c', 'D', 'd', 'E', 'e', 'F', 'f', 'G', 'g', 'H', 'h', 'I', 'i', 'J',
    'j', 'K', 'k', 'L', 'l', 'M', 'm', 'N', 'n', 'O', 'o', 'P', 'p', 'Q', 'q', 'R', 'r', 'S', 's',
    'T', 't', 'U', 'u', 'V', 'v', 'W', 'w', 'X', 'x', 'Y', 'y', 'Z', 'z',
];

const NEXT_LETTERS: [char; 63] = [
    'A', 'a', 'B', 'b', 'C', 'c', 'D', 'd', 'E', 'e', 'F', 'f', 'G', 'g', 'H', 'h', 'I', 'i', 'J',
    'j', 'K', 'k', 'L', 'l', 'M', 'm', 'N', 'n', 'O', 'o', 'P', 'p', 'Q', 'q', 'R', 'r', 'S', 's',
    'T', 't', 'U', 'u', 'V', 'v', 'W', 'w', 'X', 'x', 'Y', 'y', 'Z', 'z', '1', '2', '3', '4', '5',
    '6', '7', '8', '9', '0', '_',
];

/// Returns minified WGSL string
#[wasm_bindgen]
pub fn minify(source: &str) -> Result<String, String> {
    let mut module = wgsl_front::parse_str(source).map_err(|e| e.to_string())?;

    compact(&mut module, KeepUnused::No);
    remove_identifiers(&mut module);

    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|e| e.to_string())?;

    let wgsl = wgsl_back::write_string(&module, &info, wgsl_back::WriterFlags::empty())
        .map_err(|e| e.to_string())?;

    let res_wgsl = rename_e_temps(&wgsl);

    Ok(minify_wgsl(&res_wgsl))
}

#[inline]
fn name_from_count(count: &mut usize) -> String {
    let mut id = *count;
    *count += 1;

    // 1-character names
    if id < FIRST_LETTERS.len() {
        return FIRST_LETTERS[id].to_string();
    }
    id -= FIRST_LETTERS.len();

    // 2-character names
    let first = FIRST_LETTERS[id % FIRST_LETTERS.len()];
    id /= FIRST_LETTERS.len();
    let second = NEXT_LETTERS[id % NEXT_LETTERS.len()];

    let mut name = String::new();
    name.push(first);
    name.push(second);

    // N-character names
    id /= NEXT_LETTERS.len();
    while id != 0 {
        name.push(NEXT_LETTERS[id % NEXT_LETTERS.len()]);
        id /= NEXT_LETTERS.len();
    }

    name
}

fn remove_type_identifiers(
    count: &mut usize,
    ty: &Type,
    map: &HashMap<Handle<Type>, Handle<Type>>,
) -> Type {
    Type {
        name: Some(name_from_count(count)),
        inner: match ty.inner.clone() {
            TypeInner::Pointer { base, space } => TypeInner::Pointer {
                base: map[&base],
                space,
            },
            TypeInner::Array { base, size, stride } => TypeInner::Array {
                base: map[&base],
                size,
                stride,
            },
            TypeInner::Struct { members, span } => TypeInner::Struct {
                members: members
                    .into_iter()
                    .map(|m| StructMember {
                        name: Some(name_from_count(count)),
                        ty: map[&m.ty],
                        binding: m.binding,
                        offset: m.offset,
                    })
                    .collect(),
                span,
            },
            TypeInner::BindingArray { base, size } => TypeInner::BindingArray {
                base: map[&base],
                size,
            },
            other => other,
        },
    }
}

fn remove_fn_identifiers(
    count: &mut usize,
    function: &mut Function,
    ty_map: &HashMap<Handle<Type>, Handle<Type>>,
) {
    function.name = Some(name_from_count(count));

    if let Some(res) = function.result.as_mut() {
        res.ty = ty_map[&res.ty];
    }

    for (_, v) in function.local_variables.iter_mut() {
        v.name = Some(name_from_count(count));
        v.ty = ty_map[&v.ty];
    }

    for arg in function.arguments.iter_mut() {
        arg.name = Some(name_from_count(count));
        arg.ty = ty_map[&arg.ty];
    }

    function.named_expressions.clear();
}

fn remove_identifiers(module: &mut Module) {
    let mut count = 0;
    let mut new_types = UniqueArena::new();
    let mut ty_map = HashMap::new();

    for (h, ty) in module.types.iter() {
        let new_ty = remove_type_identifiers(&mut count, ty, &ty_map);
        let new_h = new_types.insert(new_ty, module.types.get_span(h));
        ty_map.insert(h, new_h);
    }

    module.types = new_types;

    for (_, c) in module.constants.iter_mut() {
        if !matches!(
            module.global_expressions.try_get(c.init).unwrap(),
            Expression::Override(_)
        ) {
            c.name = None;
        }
        c.ty = ty_map[&c.ty];
    }

    for (_, g) in module.global_variables.iter_mut() {
        g.name = Some(name_from_count(&mut count));
        g.ty = ty_map[&g.ty];
    }

    for (_, f) in module.functions.iter_mut() {
        remove_fn_identifiers(&mut count, f, &ty_map);
    }

    for ep in module.entry_points.iter_mut() {
        remove_fn_identifiers(&mut count, &mut ep.function, &ty_map);
    }
}

#[inline]
fn ends_with_keyword(out: &str) -> bool {
    matches!(
        out,
        s if s.ends_with("let")
            || s.ends_with("return")
            || s.ends_with("var")
            || s.ends_with("fn")
            || s.ends_with("struct")
            || s.ends_with("override")
            || s.ends_with("const")
    )
}

#[inline]
fn next_non_ws(chars: &[char], mut i: usize) -> char {
    while i < chars.len() {
        if !chars[i].is_whitespace() {
            return chars[i];
        }
        i += 1;
    }
    ' '
}

fn minify_wgsl(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut last = ' ';

    let mut in_attribute = false;
    let mut finished_attribute = false;

    let chars: Vec<char> = src.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied().unwrap_or(' ');

        // Start of attribute
        if c == '@' {
            in_attribute = true;
            finished_attribute = false;
            out.push(c);
            last = c;
            continue;
        }

        // Track attribute name characters
        if in_attribute {
            if c.is_ascii_alphanumeric() || c == '_' {
                out.push(c);
                last = c;
                continue;
            } else {
                in_attribute = false;
                finished_attribute = true;
            }
        }

        if finished_attribute && c == 'f' && next == 'n' {
            out.push(' ');
            finished_attribute = false;
        }

        if c.is_whitespace() {
            // Preserve space after WGSL keywords
            if ends_with_keyword(&out) {
                out.push(' ');
            } else if (last.is_ascii_alphanumeric() && next.is_ascii_alphanumeric())
                || (last == '-' && (next.is_ascii_digit() || next == '.'))
            {
                out.push(' ');
            }

            last = ' ';
            continue;
        }

        // Remove trailing commas
        if c == ',' {
            let nn = next_non_ws(&chars, i + 1);
            if matches!(nn, '}' | ')' | ']') {
                last = c;
                continue;
            }
        }

        out.push(c);
        last = c;

        finished_attribute = false;
    }

    out
}

fn rename_e_temps(wgsl: &str) -> String {
    let mut used = BTreeSet::<String>::new();

    let bytes = wgsl.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;

            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }

            used.insert(wgsl[start..i].to_string());
        } else {
            i += 1;
        }
    }

    let mut next_name = 0usize;
    let mut make_name = || {
        loop {
            let mut n = next_name;
            next_name += 1;

            let mut name = String::new();

            name.push((b'a' + (n % 26) as u8) as char);
            n /= 26;

            while n != 0 {
                name.push((b'a' + (n % 26) as u8) as char);
                n /= 26;
            }

            if !used.contains(&name) && !is_wgsl_keyword(&name) {
                used.insert(name.clone());
                return name;
            }
        }
    };

    let mut rename_map = HashMap::<String, String>::new();

    let mut out = String::with_capacity(wgsl.len());
    let mut i = 0;

    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;

            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }

            let ident = &wgsl[start..i];

            if is_e_temp(ident) {
                let new_name = match rename_map.get(ident) {
                    Some(name) => name.clone(),
                    None => {
                        let name = make_name();
                        rename_map.insert(ident.to_string(), name.clone());
                        name
                    }
                };

                out.push_str(&new_name);
            } else {
                out.push_str(ident);
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn is_e_temp(name: &str) -> bool {
    let bytes = name.as_bytes();

    bytes.len() > 2
        && bytes[0] == b'_'
        && bytes[1] == b'e'
        && bytes[2..].iter().all(|c| c.is_ascii_digit())
}

fn is_wgsl_keyword(name: &str) -> bool {
    matches!(
        name,
        "alias"
            | "break"
            | "case"
            | "const"
            | "continuing"
            | "continue"
            | "default"
            | "discard"
            | "else"
            | "enable"
            | "fn"
            | "for"
            | "if"
            | "let"
            | "loop"
            | "override"
            | "return"
            | "struct"
            | "switch"
            | "var"
            | "while"
            | "atomic"
            | "bool"
            | "f16"
            | "f32"
            | "i32"
            | "u32"
            | "vec2"
            | "vec3"
            | "vec4"
            | "mat2x2"
            | "mat2x3"
            | "mat2x4"
            | "mat3x2"
            | "mat3x3"
            | "mat3x4"
            | "mat4x2"
            | "mat4x3"
            | "mat4x4"
            | "array"
            | "ptr"
            | "sampler"
            | "texture_1d"
            | "texture_2d"
            | "texture_2d_array"
            | "texture_3d"
            | "texture_cube"
            | "texture_cube_array"
            | "texture_multisampled_2d"
            | "texture_storage_1d"
            | "texture_storage_2d"
            | "texture_storage_2d_array"
            | "texture_storage_3d"
    )
}
