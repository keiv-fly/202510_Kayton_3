use kayton_api::{
    KayCtx, KayError, KayErrorCode, KayExtension, KayHandle, KayResult, KayValueKind,
};
use kayton_plugin_macros::kayton_extension;

#[kayton_extension(
    name = "print",
    doc = "Print a value to stdout using the host formatter."
)]
pub fn print(_ctx: &KayCtx, value: KayHandle) -> KayResult<()> {
    let formatted = format_value(&value)?;
    println!("{formatted}");
    Ok(())
}

#[kayton_extension(name = "len", doc = "Return the length of a string or bytes value.")]
pub fn len(_ctx: &KayCtx, value: KayHandle) -> KayResult<i64> {
    match value.describe()? {
        KayValueKind::String(data) => Ok(data.len() as i64),
        KayValueKind::Bytes(data) => Ok(data.len() as i64),
        other => Err(KayError::new(
            KayErrorCode::TypeMismatch,
            format!("len is not defined for {other:?}"),
        )),
    }
}

fn format_value(handle: &KayHandle) -> KayResult<String> {
    match handle.describe()? {
        KayValueKind::Int(value) => Ok(value.to_string()),
        KayValueKind::Bool(value) => Ok(value.to_string()),
        KayValueKind::String(data) => Ok(data.to_string()),
        KayValueKind::Bytes(data) => Ok(format!("bytes[{}]", data.len())),
        KayValueKind::Unit => Ok("()".to_string()),
        KayValueKind::Capsule { tag } => Ok(format!("<capsule {tag}>", tag = tag)),
    }
}

pub fn extensions() -> &'static [KayExtension] {
    &[PRINT_EXTENSION, LEN_EXTENSION]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kayton_api::ToKay;
    use kayton_host::KayHost;

    #[test]
    fn print_and_len_extensions_work() {
        let host = KayHost::new();
        host.register_extensions(extensions()).expect("register");
        let ctx = host.api_ctx();
        let hello = "hello".to_kay(&ctx).expect("alloc");
        let length = len(&ctx, hello.clone()).expect("len");
        assert_eq!(length, 5);
        // print should not fail
        print(&ctx, hello).expect("print");
    }
}
