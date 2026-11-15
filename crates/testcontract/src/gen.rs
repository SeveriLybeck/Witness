//! xUnit tests generated from a replay input, as a suggested patch (D28, playbook C5).
//!
//! Template: the replay text's `// lib` section is compiled to an in-memory assembly and
//! referenced as metadata by the `// app` section; the source generator runs on the app
//! compilation and the test asserts the warning-or-worse diagnostic IDs that the new code gave
//! on the replay (`replays.json` `new`). Written in the repository's own test style (xUnit,
//! `ReferencesHelper`, `ReferenceAssemblies`).

/// Split a replay text into (lib, app) sources at `// lib` and `// app` marker lines.
pub fn split_replay(text: &str) -> (Option<String>, String) {
    let mut lib = Vec::new();
    let mut app = Vec::new();
    let mut cur: Option<bool> = None; // Some(true) = lib
    for l in text.lines() {
        match l.trim() {
            "// lib" => cur = Some(true),
            "// app" => cur = Some(false),
            _ => match cur {
                Some(true) => lib.push(l),
                _ => app.push(l),
            },
        }
    }
    let lib = if lib.is_empty() { None } else { Some(lib.join("\n")) };
    (lib, app.join("\n"))
}

/// Diagnostic IDs (MsgPack004, CS8785, AD0001) named in an outcome text; "0 warnings" names none.
pub fn diagnostic_ids(outcome: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in outcome.split(|c: char| !c.is_ascii_alphanumeric()) {
        let letters: String = w.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let digits = &w[letters.len()..];
        if letters.len() >= 2 && letters.chars().next().unwrap().is_ascii_uppercase() && digits.len() >= 3 && digits.chars().all(|c| c.is_ascii_digit()) && !out.contains(&w.to_string()) {
            out.push(w.to_string());
        }
    }
    out.sort();
    out
}

fn raw(s: &str) -> String {
    // a raw string literal whose quote run is longer than any run inside
    let mut longest = 0;
    let mut run = 0;
    for c in s.chars() {
        if c == '"' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let q = "\"".repeat((longest + 1).max(3));
    format!("{q}\n{s}\n{q}")
}

/// One generated test file for difference `number` (`id` like `d2`) on block `block`.
pub fn test_source(id: &str, number: u64, block: &str, replay_text: &str, new_outcome: &str) -> (String, String) {
    let class = format!("WitnessDifference{number}Tests");
    let (lib, app) = split_replay(replay_text);
    let ids = diagnostic_ids(new_outcome);
    let expected = if ids.is_empty() {
        "System.Array.Empty<string>()".to_string()
    } else {
        format!("new[] {{ {} }}", ids.iter().map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(", "))
    };
    let lib_block = match &lib {
        Some(l) => format!(
            "        string libSource = {};\n        MetadataReference libReference = CompileToReference(\"Lib\", libSource, references);\n        references = references.Append(libReference).ToArray();\n",
            indent(&raw(l))
        ),
        None => String::new(),
    };
    let src = format!(
        r#"// Copyright (c) All contributors. All rights reserved.
// Licensed under the MIT license. See LICENSE file in the project root for full license information.

// Suggested by Witness from the replay input of difference {id} (block {block}).

using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.Testing;

public class {class}
{{
    [Fact]
    public async Task Difference{number}_ReplayInput()
    {{
        ImmutableArray<MetadataReference> framework = await ReferenceAssemblies.Net.Net80.ResolveAsync(LanguageNames.CSharp, CancellationToken.None);
        MetadataReference[] references = framework.Concat(ReferencesHelper.GetReferences(ReferencesSet.MessagePack | ReferencesSet.MessagePackAnnotations)).ToArray();
{lib_block}        string appSource = {app_lit};
        Compilation app = CSharpCompilation.Create("App", new[] {{ CSharpSyntaxTree.ParseText(appSource, ParseOptions) }}, references, new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary));
        GeneratorDriverRunResult runResult = CSharpGeneratorDriver.Create(new[] {{ new MessagePack.SourceGenerator.MessagePackGenerator().AsSourceGenerator() }}, parseOptions: ParseOptions)
            .RunGenerators(app)
            .GetRunResult();
        string[] reported = runResult.Diagnostics.Where(d => d.Severity >= DiagnosticSeverity.Warning).Select(d => d.Id).Distinct().OrderBy(d => d, StringComparer.Ordinal).ToArray();
        Assert.Equal({expected}, reported);
    }}

    private static readonly CSharpParseOptions ParseOptions = CSharpParseOptions.Default.WithLanguageVersion(LanguageVersion.Latest);

    private static MetadataReference CompileToReference(string assemblyName, string source, IEnumerable<MetadataReference> references)
    {{
        Compilation compilation = CSharpCompilation.Create(assemblyName, new[] {{ CSharpSyntaxTree.ParseText(source, ParseOptions) }}, references, new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary));
        using var stream = new MemoryStream();
        Microsoft.CodeAnalysis.Emit.EmitResult result = compilation.Emit(stream);
        Assert.True(result.Success, string.Join(Environment.NewLine, result.Diagnostics));
        return MetadataReference.CreateFromImage(stream.ToArray());
    }}
}}
"#,
        app_lit = indent(&raw(&app)),
    );
    (class, src)
}

fn indent(s: &str) -> String {
    // raw string content lines must start with the closing quotes' indentation; use none
    s.to_string()
}

/// A `git apply` patch adding one new file.
pub fn new_file_patch(path: &str, text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut p = format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n", lines.len());
    for l in lines {
        p.push('+');
        p.push_str(l);
        p.push('\n');
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_and_split() {
        assert_eq!(diagnostic_ids("CS8785, AD0001: IndexOutOfRangeException"), vec!["AD0001", "CS8785"]);
        assert!(diagnostic_ids("0 warnings; formatter without Key").is_empty());
        assert_eq!(diagnostic_ids("MsgPack004: global::Lib.S<int>.Unkeyed"), vec!["MsgPack004"]);
        let (l, a) = split_replay("// lib\nclass L {}\n// app\nclass A {}");
        assert_eq!(l.as_deref(), Some("class L {}"));
        assert_eq!(a, "class A {}");
        let (_, src) = test_source("d2", 2, "b2", "// lib\nclass L {}\n// app\nclass A {}", "0 warnings");
        assert!(src.contains("public class WitnessDifference2Tests"));
        assert!(src.contains("System.Array.Empty<string>()"));
        let p = new_file_patch("t/X.cs", "a\nb\n");
        assert!(p.ends_with("@@ -0,0 +1,2 @@\n+a\n+b\n"));
    }
}
