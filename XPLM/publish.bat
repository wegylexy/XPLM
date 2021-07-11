setlocal
cd /d %~dp0
dotnet publish /p:NativeLib=Shared /p:SelfContained=True -r win-x64 -c Release
cd bin\Release\net6.0\win-x64\publish
move /Y FlyByWireless.XPLM.dll win.xpl