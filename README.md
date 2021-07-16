# FlyByWireless.XPLM
P/Invoke for X-Plane plugin library manager

## Reference Packages
`Microsoft.DotNet.ILCompiler` must be a top level dependency for native AOT:
```xml
<PackageReference Include="FlyByWireless.XPLM" Version="1.0.*-*"/>
<PackageReference Include="Microsoft.DotNet.ILCompiler" Version="6.0.*-*"/>
```
## Implement
The plugin must implement `FlyByWireless.XPLM.XPluginBase` as `XPlugin` in its assembly root namespace:
```cs
using FlyByWireless.XPLM;
using System;

namespace XplTemplate
{
    sealed class XPlugin : XPluginBase
    {
        public override string? Name => "Fly by Wireless";
        public override string? Signature => "hk.timtim.flybywireless";
        public override string? Description => "X-Plane plugin library template.";

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

        public override bool Enable()
        {
            // TODO: start loops
            return true;
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