using MessagePack;
namespace Lib;
public static class Marker { }

[MessagePackObject]
public struct S<T>
{
    [Key(0)] public string Key { get; set; }
    public int Unkeyed { get; set; }   // public, no [Key], no [IgnoreMember]
}
