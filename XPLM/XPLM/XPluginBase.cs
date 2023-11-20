namespace FlyByWireless.XPLM;

/// <summary>
/// <see href="https://developer.x-plane.com/article/developing-plugins/#Basic_Plugin_Reference"/>
/// </summary>
public abstract class XPluginBase : IDisposable
{
    /// <summary>
    /// <para>Human-readable name of the plugin.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginStart">XPluginStart</seealso>
    /// </summary>
    public virtual ReadOnlySpan<byte> Name => null;

    /// <summary>
    /// <para>Globally unique signature of the plugin.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginStart">XPluginStart</seealso>
    /// </summary>
    public virtual ReadOnlySpan<byte> Signature => null;

    /// <summary>
    /// <para>Human-readable description of the plugin.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginStart">XPluginStart</seealso>
    /// </summary>
    public virtual ReadOnlySpan<byte> Description => "Built with FlyByWireless.XPLM"u8;

    /// <summary>
    /// <para>Do any initialization necessary for the plugin. This includes creating user interfaces, installing registered callbacks, allocating resources, etc.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginStart">XPluginStart</seealso>
    /// </summary>
    public XPluginBase() { }

    ~XPluginBase() => Dispose(false);

    /// <summary>
    /// <para>Unregister any callbacks that can be unregistered, dispose of any objects or resources, and clean up all allocations done by the plugin.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginStop">XPluginStop</seealso>
    /// </summary>
    /// <param name="isDisposing">Whether it is disposing or finalizing.</param>
    protected virtual void Dispose(bool isDisposing) { }

    public void Dispose()
    {
        Dispose(true);
        GC.SuppressFinalize(this);
    }

    /// <summary>
    /// <para>This callback should be used to allocate any resources that the plugin maintains while enabled.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginEnable">XPluginEnable</seealso>
    /// </summary>
    public abstract void Enable();

    /// <summary>
    /// <para>Deallocate any significant resources and prepare to not receive any callbacks for a potentially long duration.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginDisable">XPluginDisable</seealso>
    /// </summary>
    public abstract void Disable();

    /// <summary>
    /// <para>This function is called by the plugin manager when a message is sent to the plugin.</para>
    /// See also <seealso href="https://developer.x-plane.com/article/developing-plugins/#XPluginReceiveMessage">XPluginReceiveMessage</seealso>
    /// </summary>
    /// <param name="from">The ID of the plugin that sent the message.</param>
    /// <param name="message">An integer indicating the message sent.</param>
    /// <param name="param">A pointer to data that is specific to the message.</param>
    public virtual void ReceiveMessage(int from, int message, nint param) { }
}