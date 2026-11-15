using MessagePack; using Lib;
namespace App;

[MessagePackObject]
public sealed class Holder
{
    [Key(0)] public S<int> Member { get; set; }
}
public static class Probe
{
    public static object Make() => new Holder { Member = new S<int> { Key = "k", Unkeyed = 42 } };
    public static string Show(object o) { var h = (Holder)o; return $"Holder {{ Member = S<int> {{ Key = {h.Member.Key}, Unkeyed = {h.Member.Unkeyed} }} }}"; }
}
