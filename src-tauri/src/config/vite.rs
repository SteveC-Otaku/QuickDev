//! Conservative literal-only inspection. Never import or execute project config.
use std::{fs, path::Path};

pub fn read_address(root: &Path) -> Result<(u16, String), String> {
    let path = ["vite.config.ts", "vite.config.js", "vite.config.mts", "vite.config.mjs", "vite.config.cts", "vite.config.cjs"]
        .iter().map(|name| root.join(name)).find(|path| path.is_file())
        .ok_or("未找到 Vite 配置；请填写网页端口或地址")?;
    if fs::metadata(&path).map_err(|_| "无法读取 Vite 配置")?.len() > 1_048_576 {
        return Err("Vite 配置过大；请填写网页端口或地址".into());
    }
    let raw = fs::read_to_string(path).map_err(|_| "无法读取 Vite 配置")?;
    parse_address(&raw).ok_or_else(|| "无法静态识别 Vite 的 host/port；请填写网页端口或地址".into())
}

fn tokens(raw: &str) -> Option<Vec<String>> {
    let chars: Vec<_> = raw.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        match chars[i] {
            c if c.is_whitespace() => { i += 1; }
            '/' if chars.get(i+1)==Some(&'/') => {
                i += 2;
                while i < chars.len() && chars[i]!='\n' { i += 1; }
            }
            '/' if chars.get(i+1)==Some(&'*') => {
                i += 2;
                while i+1 < chars.len() && !(chars[i]=='*' && chars[i+1]=='/') { i += 1; }
                if i+1 >= chars.len() { return None; }
                i += 2;
            }
            quote @ ('\'' | '"' | '`') => {
                i += 1;
                while i < chars.len() && chars[i]!=quote {
                    if chars[i]=='\\' { i += 1; }
                    i += 1;
                }
                if i >= chars.len() { return None; }
                i += 1;
                out.push(chars[start..i].iter().collect());
            }
            c if c.is_alphanumeric() || c=='_' || c=='$' => {
                i += 1;
                while i < chars.len() && (chars[i].is_alphanumeric() || matches!(chars[i], '_' | '$')) { i += 1; }
                out.push(chars[start..i].iter().collect());
            }
            c => { out.push(c.to_string()); i += 1; }
        }
    }
    Some(out)
}

// Return only an unambiguous direct property; spreads/computed keys may override it.
fn property(tokens: &[String], object: usize, key: &str) -> Option<Option<usize>> {
    let mut depth = 1;
    let mut found = None;
    let mut boundary = true;
    let mut i = object + 1;
    while i < tokens.len() && depth > 0 {
        let token = tokens[i].as_str();
        if depth == 1 {
            if boundary && (token=="." || token=="[") { return None; }
            if boundary && (token==key || token==format!("'{key}'") || token==format!("\"{key}\"")) && tokens.get(i+1).is_some_and(|s| s==":") {
                if found.is_some() { return None; }
                found = Some(i+2);
            }
            boundary = token == ",";
        }
        match token { "{" | "[" | "(" => depth += 1, "}" | "]" | ")" => depth -= 1, _ => {} }
        i += 1;
    }
    (depth==0).then_some(found)
}

pub(super) fn parse_address(raw: &str) -> Option<(u16, String)> {
    let tokens = tokens(raw)?;
    let export = tokens.windows(2).position(|pair| pair[0]=="export" && pair[1]=="default")? + 2;
    let object = if tokens.get(export)?=="{" { export }
        else if tokens.get(export)?=="defineConfig" && tokens.get(export+1)?=="(" && tokens.get(export+2)?=="{" { export+2 }
        else { return None; };
    let server = property(&tokens, object, "server")??;
    if tokens.get(server)? != "{" { return None; }
    if let Some(https) = property(&tokens, server, "https")? {
        if tokens.get(https)? != "false" || !matches!(tokens.get(https+1)?.as_str(), "," | "}") { return None; }
    }
    let port = property(&tokens, server, "port")??;
    if !matches!(tokens.get(port+1)?.as_str(), "," | "}") { return None; }
    let port: u16 = tokens.get(port)?.parse().ok().filter(|p| *p>0)?;
    let host = match property(&tokens, server, "host")? {
        None => "localhost".to_string(),
        Some(index) => {
            if !matches!(tokens.get(index+1)?.as_str(), "," | "}") { return None; }
            let value = tokens.get(index)?;
            if value=="true" || value=="false" { "localhost".into() }
            else if (value.starts_with('\'') && value.ends_with('\'')) || (value.starts_with('"') && value.ends_with('"')) {
                let host = &value[1..value.len()-1];
                if host=="0.0.0.0" || host=="::" { "localhost".into() }
                else if !host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || ".:-".contains(c)) { host.into() }
                else { return None; }
            } else { return None; }
        }
    };
    let host = if host.contains(':') { format!("[{host}]") } else { host };
    Some((port, format!("http://{host}:{port}/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_literals_without_executing_config() {
        assert_eq!(parse_address("export default defineConfig({plugins:[react()],server:{host:'127.0.0.1',port:5174,strictPort:true,proxy:{'/api':{target:'http://localhost:8000'}}}})"), Some((5174,"http://127.0.0.1:5174/".into())));
        assert_eq!(parse_address("export default { server: { port: 4321, host: '127.0.0.2' } }"), Some((4321,"http://127.0.0.2:4321/".into())));
        assert_eq!(parse_address("export default { server: { port: 4321, host: '::1' } }"), Some((4321,"http://[::1]:4321/".into())));
        for host in ["true", "'0.0.0.0'", "'::'"] {
            assert_eq!(parse_address(&format!("export default {{server:{{host:{host},port:3005}}}}")),Some((3005,"http://localhost:3005/".into())));
        }
    }
    #[test]
    fn refuses_computed_overrides_and_comment_decoys() {
        for raw in ["export default defineConfig(()=>({server:{port:5174}}))", "export default {server:{port:Number(process.env.PORT)}}", "export default {server:{port:5174+1}}", "export default {server:{port:5174,host:process.env.HOST}}", "export default {...shared,server:{port:5174}}", "export default {server:{port:5174,...shared}}", "export default {server:{port:5174,port:8000}}", "export default {server:{port:0}}", "export default {server:{port:65536}}", "// server:{port:5174}\nexport default {}", "export default {note:'server: {port:5174}'}"] {
            assert_eq!(parse_address(raw),None,"{raw}");
        }
        assert_eq!(parse_address("export default { /* port:8000 */ server:{port:6123} }"),Some((6123,"http://localhost:6123/".into())));
    }
}
