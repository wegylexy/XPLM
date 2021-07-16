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
}