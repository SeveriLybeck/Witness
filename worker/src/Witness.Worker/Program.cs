using System.Text.Json;
using System.Text.Json.Nodes;

namespace Witness.Worker;

/// The C# worker: one worker-request per stdin line, one worker-reply per stdout line (contracts/messages/worker-*.schema.json).
public static class Worker
{
    public static JsonObject Handle(JsonObject req)
    {
        var id = (int?)req["id"] ?? 0;
        var method = (string?)req["method"] ?? "";
        var known = method is "cfg" or "references" or "dependencies" or "facts" or "run";
        var reply = new JsonObject { ["id"] = id < 1 ? 1 : id, ["method"] = known ? method : "cfg" };
        try
        {
            var p = req["params"]?.AsObject() ?? throw new ArgumentException("params missing");
            foreach (var key in new[] { "checkout", "old", "new" })
                if (method != "run" && p[key] is JsonObject co) Checkout.Verify(co);
            reply["result"] = method switch
            {
                "cfg" => Cfg.Run(p),
                "references" => Links.References(p),
                "dependencies" => Links.Dependencies(p),
                "facts" => Facts.Run(p),
                "run" => Lab.Run(p),
                _ => throw new ArgumentException($"unknown method {method}"),
            };
        }
        catch (Exception e)
        {
            reply.Remove("result");
            reply["error"] = new JsonObject { ["message"] = $"{e.GetType().Name}: {e.Message}" };
        }
        return reply;
    }

    public static string HandleLine(string line)
    {
        JsonObject req;
        try { req = JsonNode.Parse(line)!.AsObject(); }
        catch (Exception e)
        {
            return ErrorReply(1, "cfg", "unparseable request: " + e.Message);
        }
        var id = 1;
        var method = "cfg";
        try
        {
            id = (int?)req["id"] ?? 1;
            method = (string?)req["method"] ?? "cfg";
            return Handle(req).ToJsonString(Json);
        }
        catch (Exception e)
        {
            return ErrorReply(id, method, $"{e.GetType().Name}: {e.Message}");
        }
    }

    static readonly JsonSerializerOptions Json = new()
    {
        WriteIndented = false,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        TypeInfoResolver = new System.Text.Json.Serialization.Metadata.DefaultJsonTypeInfoResolver(),
    };

    static string ErrorReply(int id, string method, string message)
    {
        if (id < 1) id = 1;
        if (method is not ("cfg" or "references" or "dependencies" or "facts" or "run")) method = "cfg";
        return new JsonObject { ["id"] = id, ["method"] = method, ["error"] = new JsonObject { ["message"] = message } }.ToJsonString(Json);
    }

    public static int Main(string[] args)
    {
        var stdout = Console.Out;
        Console.SetOut(Console.Error); // nothing but replies on stdout
        string? line;
        while ((line = Console.In.ReadLine()) != null)
        {
            if (line.Trim().Length == 0) continue;
            stdout.WriteLine(HandleLine(line));
            stdout.Flush();
        }
        return 0;
    }
}
