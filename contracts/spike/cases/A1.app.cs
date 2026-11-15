using MessagePack; using Lib;
namespace App;

[MessagePackObject]
public sealed partial record Cmd([property: Key(0)] string Scope)
{
    [Key(1)] public Item<int> Member { get; init; }
}
public static class Probe
{
    public static object Make() => new Cmd("s") { Member = new Item<int>("k") };
    public static string Show(object o) => o.ToString()!;
}
