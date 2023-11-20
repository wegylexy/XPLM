# FlyByWireless.XPLM
P/Invoke for X-Plane plugin library manager

## Project Setup
```xml
<Project Sdk="Microsoft.NET.Sdk">
    <PropertyGroup>
        <PublishAot>true</PublishAot>
    </PropertyGroup>
    <ItemGroup>
        <PackageReference Include="FlyByWireless.XPLM" Version="1.0.0-*" />
    </ItemGroup>
</Project>
```

## Implement
The plugin must implement `FlyByWireless.XPLM.XPluginBase` as `XPlugin` in its assembly root namespace:
```cs
using FlyByWireless.XPLM;
using FlyByWireless.XPWidgets;

namespace XplTemplate;

sealed class XPlugin : XPluginBase
{
    public override ReadOnlySpan<byte> Name => "Fly by Wireless"u8;
    public override ReadOnlySpan<byte> Signature => "tw.timmywong.flybywireless"u8;
    public override ReadOnlySpan<byte> Description => "X-Plane plugin library template."u8;

    public XPlugin() : base()
    {
        // e.g. check for API support
        if (Utilities.Versions.XPLMVersion < 303)
        {
            throw new NotSupportedException("TCAS override not supported.");
        }
    }

    public override void Dispose()
    {
        // TODO: uninitialize
    }

    public override void Enable()
    {
        // TODO: start loops
    }

    public override void Disable()
    {
        // TODO: stop loops
    }

    public override void ReceiveMessage(int from, int message, nint param)
    {
        // TODO: handle message from aother plugin
        base.ReceiveMessage(from, message, param);
    }
}
```
`Name` and `Signature` fall back to assembly name and root namespace respectively when not overridden to return non-`null` values.
## Publish
```bat
dotnet publish -r win-x64 -c Release
```
```sh
dotnet publish -r osx-x64 -c Release
```
```sh
dotnet publish -r linux-x64 -c Release
```
In `Debug` config, plugin error messages are written to Debug Output; if a debugger is attached, it will also break. An error callback could be installed in `Release` config too:
```cs
Utilities.ErrorCallback = message =>
{
    Debug.WriteLine(message);
    if (Debugger.IsAttached)
    {
        Debugger.Break();
    }
};
```