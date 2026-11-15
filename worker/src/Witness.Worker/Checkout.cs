using System.Text.Json.Nodes;

namespace Witness.Worker;

/// A request's checkout must be at the commit it names: HEAD is read from the worktree's git files (no git process).
public static class Checkout
{
    public static void Verify(JsonObject co)
    {
        var wt = (string?)co["worktree"] ?? throw new ArgumentException("checkout.worktree missing");
        var commit = (string?)co["commit"];
        if (!Directory.Exists(wt)) throw new ArgumentException($"worktree {wt} does not exist");
        if (string.IsNullOrEmpty(commit)) return;
        var head = Head(wt);
        if (!string.Equals(head, commit, StringComparison.OrdinalIgnoreCase))
            throw new ArgumentException($"worktree {wt} is at {head ?? "unknown"}, request names {commit}");
    }

    public static string? Head(string wt)
    {
        var dotGit = Path.Combine(wt, ".git");
        string gitDir;
        if (Directory.Exists(dotGit)) gitDir = dotGit;
        else if (File.Exists(dotGit))
        {
            var line = File.ReadAllText(dotGit).Trim();
            if (!line.StartsWith("gitdir:", StringComparison.Ordinal)) return null;
            gitDir = Path.GetFullPath(Path.Combine(wt, line[7..].Trim()));
        }
        else return null;
        var headFile = Path.Combine(gitDir, "HEAD");
        if (!File.Exists(headFile)) return null;
        var head = File.ReadAllText(headFile).Trim();
        if (!head.StartsWith("ref:", StringComparison.Ordinal)) return head;
        var refName = head[4..].Trim();
        // refs live in the common dir for linked worktrees
        var common = File.Exists(Path.Combine(gitDir, "commondir"))
            ? Path.GetFullPath(Path.Combine(gitDir, File.ReadAllText(Path.Combine(gitDir, "commondir")).Trim())) : gitDir;
        foreach (var d in new[] { gitDir, common })
        {
            var f = Path.Combine(d, refName);
            if (File.Exists(f)) return File.ReadAllText(f).Trim();
        }
        var packed = Path.Combine(common, "packed-refs");
        if (File.Exists(packed))
            foreach (var l in File.ReadLines(packed))
                if (l.EndsWith(" " + refName, StringComparison.Ordinal)) return l.Split(' ')[0];
        return null;
    }
}
