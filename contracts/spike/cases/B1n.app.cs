using MessagePack;
namespace App;
[MessagePackObject]
public class T { [Key(0)] public int A { get; set; } public int B { get; set; } }
public static class Probe
{
    public static object Make() => new T { A = 1, B = 2 };
    public static string Show(object o) { var x = (T)o; return $"T {{ A = {x.A}, B = {x.B} }}"; }
}
