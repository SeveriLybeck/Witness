using MessagePack;
namespace Lib;
public static class Marker { }

[MessagePackObject]
public partial record struct Item<T>([property: Key(0)] string Key);
