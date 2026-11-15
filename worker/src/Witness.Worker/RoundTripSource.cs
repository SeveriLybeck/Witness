namespace Witness.Worker;

/// Entry point compiled into the reviewer's app for a round trip; it runs only in a bwrap child and prints one JSON line.
static class RoundTripSource
{
    public const string Text = """
#nullable disable
namespace WitnessRoundTrip
{
    internal static class Runner
    {
        static string Show(object o)
        {
            if (o is null) return "null";
            try { return System.Text.Json.JsonSerializer.Serialize(o, o.GetType(), new System.Text.Json.JsonSerializerOptions { IncludeFields = true }); }
            catch (System.Exception e) { return o.ToString() + " (" + e.GetType().Name + ")"; }
        }

        static string Esc(string s) => s is null ? "null" : System.Text.Json.JsonSerializer.Serialize(s);

        static int Main(string[] args)
        {
            string resolverName = args.Length > 0 ? args[0] : "standard";
            string sent = "", received = null, wire = null, error = null;
            try
            {
                var asm = typeof(Runner).Assembly;
                System.Type probe = null;
                foreach (var t in asm.GetTypes()) if (t.Name == "Probe" && t.GetMethod("Make", System.Type.EmptyTypes) is { IsStatic: true }) { probe = t; break; }
                if (probe is null) throw new System.InvalidOperationException("no static Probe.Make() in the app");
                object v = probe.GetMethod("Make", System.Type.EmptyTypes).Invoke(null, null);
                sent = Show(v);
                MessagePack.IFormatterResolver resolver;
                if (resolverName != "standard")
                {
                    System.Type gen = null;
                    foreach (var t in asm.GetTypes()) if (t.Name == "GeneratedMessagePackResolver") { gen = t; break; }
                    resolver = gen?.GetField("Instance", System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Static)?.GetValue(null) as MessagePack.IFormatterResolver;
                    if (resolver is null) throw new System.InvalidOperationException("no GeneratedMessagePackResolver in the app");
                    // the generated formatters look up string, int, ... through the same resolver: add the built-in primitives, never the dynamic resolver
                    if (resolverName == "generated-and-builtin") resolver = MessagePack.Resolvers.CompositeResolver.Create(resolver, MessagePack.Resolvers.BuiltinResolver.Instance);
                }
                else resolver = MessagePack.Resolvers.StandardResolver.Instance;
                var options = MessagePack.MessagePackSerializerOptions.Standard.WithResolver(resolver);
                byte[] bytes = MessagePack.MessagePackSerializer.Serialize(v.GetType(), v, options);
                wire = MessagePack.MessagePackSerializer.ConvertToJson(bytes);
                received = Show(MessagePack.MessagePackSerializer.Deserialize(v.GetType(), bytes, options));
            }
            catch (System.Exception e)
            {
                var inner = e; while (inner.InnerException is not null) inner = inner.InnerException;
                error = inner == e ? e.GetType().Name + ": " + e.Message : e.GetType().Name + ": " + inner.GetType().Name + ": " + inner.Message;
            }
            System.Console.WriteLine("{\"resolver\":" + Esc(resolverName) + ",\"sent\":" + Esc(sent) + ",\"received\":" + Esc(received) + ",\"wire\":" + Esc(wire) + ",\"error\":" + Esc(error) + "}");
            return 0;
        }
    }
}
""";
}
