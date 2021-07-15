# FlyByWireless.XPLM
P/Invoke for X-Plane plugin library manager

## Project
ILCompiler must be a top level dependency:
```xml
<PackageReference Include="FlyByWireless.XPLM" Version="1.0.0-*"/>
<PackageReference Include="Microsoft.DotNet.ILCompiler" Version="6.0.0-*"/>
```
The plugin must export native entry points:
```cs
[UnmanagedCallersOnly(EntryPoint = "XPluginStart")]
public static int Start(in byte name, in byte signature, in byte description);

[UnmanagedCallersOnly(EntryPoint = "XPluginStop")]
public static void Stop();

[UnmanagedCallersOnly(EntryPoint = "XPluginEnable")]
public static int Enable();

[UnmanagedCallersOnly(EntryPoint = "XPluginDisable")]
public static void Disable();

[UnmanagedCallersOnly(EntryPoint = "XPluginReceiveMessage")]
public static void ReceiveMessage(int from, int message, nint param);
```
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