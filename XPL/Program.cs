using FlyByWireless.XPLM;
using FlyByWireless.XPLM.DataAccess;
using FlyByWireless.XPLM.Processing;

namespace XplTemplate;

sealed class XPlugin : XPluginBase
{
    public override string? Name => "Fly by Wireless";
    public override string? Signature => "hk.timtim.flybywireless";
    public override string? Description => "X-Plane plugin library template.";

    readonly DataRef _overrideTcas;
    readonly FlightLoop _myLoop;

    public XPlugin() : base()
    {
        // e.g. check for API support
        if (Utilities.Versions.XPLMVersion < 303)
        {
            throw new NotSupportedException("TCAS override not supported.");
        }

        // Example: finds datarefs
        _overrideTcas = DataRef.Find("sim/operation/override/override_TCAS")!;
        DataRef
            bearing = DataRef.Find("sim/cockpit2/tcas/indicators/relative_bearing_degs")!,
            distance = DataRef.Find("sim/cockpit2/tcas/indicators/relative_distance_mtrs")!,
            altitude = DataRef.Find("sim/cockpit2/tcas/indicators/relative_altitude_mtrs")!;

        // Example: registers my flight loop
        _myLoop = new FlightLoop(FlightLoopPhase.AfterFlightModel, (elapsedSinceLastCall, elapsedTimeSinceLastFlightLoop, counter) =>
        {
                // TODO: set number of planes
                var count = 2;
            Span<float> values = stackalloc float[count];
                // TODO: set bearings
                bearing.AsFloatVector(0).Write(values);
                // TODO: set distances
                distance.AsFloatVector(0).Write(values);
                // TODO: set altitudes
                altitude.AsFloatVector(0).Write(values);
                // Schedules for one second later
                return 1;
        });
    }

    public override void Dispose()
    {
        // Example: unregisters my flight loop
        _myLoop.Dispose();
    }

    public override void Enable()
    {
        // Example: overrides TCAS
        _overrideTcas.AsInt = 1;

        // Example: starts my flight loop one cycle after registration, i.e. immediately
        _myLoop.Schedule(-1, false);
    }

    public override void Disable()
    {
        // Example: stops my flight loop
        _myLoop.Schedule(0, false);

        // Example: clears TCAS override
        _overrideTcas.AsInt = 0;
    }

    public override void ReceiveMessage(int from, int message, nint param)
    {
        // TODO: handle message from aother plugin
        base.ReceiveMessage(from, message, param);
    }
}