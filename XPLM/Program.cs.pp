using System.Runtime.InteropServices;
using System.Text;

namespace FlyByWireless.XPLM
{
    static class Program
    {
        private static XPluginBase? _plugin;

        [UnmanagedCallersOnly(EntryPoint = "XPluginStart")]
        private static int Start(ref byte name, ref byte signature, ref byte description)
        {
            static void StrCpy(ref byte u, string value)
            {
                var s = MemoryMarshal.CreateSpan(ref u, 256);
                s[Encoding.UTF8.GetBytes(value, s[..^1])] = 0;
            }
            try
            {
                _plugin = new $RootNamespace$.XPlugin();
                StrCpy(ref name, _plugin.Name ?? "$AssemblyName$");
                StrCpy(ref signature, _plugin.Signature ?? "$RootNamespace$");
                StrCpy(ref description, _plugin.Description ?? "Built with FlyByWireless.XPLM");
                return 1;
            }
            catch (Exception ex)
            {
                Utilities.DebugString(ex.ToString());
                return 0;
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginStop")]
        private static void Stop()
        {
            using (_plugin) 
            {
                _plugin = null;
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginEnable")]
        private static int Enable() => _plugin!.Enable() ? 1 : 0;

        [UnmanagedCallersOnly(EntryPoint = "XPluginDisable")]
        private static void Disable() => _plugin!.Disable();

        [UnmanagedCallersOnly(EntryPoint = "XPluginReceiveMessage")]
        private static void ReceiveMessage(int from, int message, nint param) =>
            _plugin!.ReceiveMessage(from, message, param);
    }
}