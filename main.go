package main // github.com/shift/fcos-mc-pi4/leds

import (
	"fmt"
	systemd "github.com/coreos/go-systemd/v22/dbus" // change namespace
	systemdUtil "github.com/coreos/go-systemd/v22/util"
	"github.com/godbus/dbus/v5" // namespace collides with systemd wrapper
	"github.com/shift/fcos-mc-pi4/leds/led"
	"github.com/shift/fcos-mc-pi4/leds/strip"
	"periph.io/x/devices/v3/nrzled"

	"flag"
	"github.com/jar-o/limlog"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
	"go.uber.org/zap"
)

var (
	logr       *limlog.Limlog
	configFile *string
)

func Configuration() {
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")
	viper.AddConfigPath(".")
	err := viper.ReadInConfig()
	if err != nil {
		panic(fmt.Errorf("fatal error config file: %w", err))
	}
	viper.WriteConfig()
}

func main() {
	Configuration()
	// First thigns first, logging...
	cfg := limlog.NewZapConfigWithLevel(zap.DebugLevel)
	logr = limlog.NewLimlogZapWithConfig(cfg)
	z := logr.L.GetLogger().(*zap.Logger)
	defer z.Sync()

	pflag.CommandLine.AddGoFlagSet(flag.CommandLine)
	pflag.Parse()
	viper.BindPFlags(pflag.CommandLine)

	spiID := flag.String("spi", "", "Use SPI MOSI implemntation")
	numPixels := flag.Int("n", 5, "number of pixels on the strip")
	hz := nrzled.DefaultOpts.Freq
	flag.Var(&hz, "s", "speed in Hz")
	channels := flag.Int("channels", 4, "number of color channels, use 4 for RGBW")
	flag.Parse()

	strip, err := strip.Init(logr, spiID, hz, numPixels, channels)

	if err != nil {
		logr.Panic("unable to initalise the strip", zap.Error(err))
	}

	if !systemdUtil.IsRunningSystemd() {
		logr.Panic("systemd is not running", zap.Error(err))
	}

	conn, err := systemd.New()

	if err != nil {
		logr.Panic("systemd unable to connect, running as root?", zap.Error(err))
	}
	err = conn.Subscribe()
	if err != nil {
		logr.Panic("systemd subscribed failed", zap.Error(err))
	}
	set := conn.NewSubscriptionSet() // no error should be returned
	services := []string{"sshd.service", "minecraft.service", "local-exporter.service", "zincati.service", "node-exporter.service"}
	for _, svc := range services {
		pixel, err := strip.Add(svc)
		if err != nil {
			logr.Panic("Error calling Strip.Add:", zap.Error(err))
		}
		go addService(conn, set, pixel)
	}
	strip.UpdateLoop()
}

func addService(conn *systemd.Conn, set *systemd.SubscriptionSet, pixelRef *led.Led) {
	subChannel, subErrors := set.Subscribe()
	pixel := *pixelRef
	var svc = pixel.Unit
	var activeSet = false
	var invalid = false
	var previous bool
	for {
		previous = invalid
		invalid = false
		loadstate, err := conn.GetUnitProperty(svc, "LoadState")
		if err != nil {
			logr.Error("Failed to get property:", zap.Error(err))
			invalid = true
		}

		if !invalid {
			var notFound = (loadstate.Value == dbus.MakeVariant("not-found"))
			if notFound {
				logr.Info("Failed to find service")
				invalid = true
			}
		}

		if previous != invalid { // if invalid changed, send signal
		}

		if invalid {
			logr.Info("Waiting for service")
			if activeSet {
				activeSet = false
				set.Remove(svc) // no return value should ever occur
			}

		} else {
			if !activeSet {
				activeSet = true
				set.Add(svc) // no return value should ever occur
			}

			select {
			case event := <-subChannel:

				// NOTE: the value returned is a map for some reason...
				if event[svc] != nil {
					switch event[svc].ActiveState {
					case "active":
						pixelRef.SetColour("00440005")
					case "inactive":
						pixelRef.SetColour("44000005")
					case "reloading":
						pixelRef.SetColour("60606060")
					case "failed":
						pixelRef.SetColour("99000000")
					case "activating":
						pixelRef.SetColour("00330010")
					case "deactivating":
						pixelRef.SetColour("22000010")
					default:
						fmt.Errorf("unknown svc state: %s", event[svc].ActiveState)
						logr.Error("Unknown service statre", zap.String("event", event[svc].ActiveState))
					}
				}

			case err := <-subErrors:
				logr.Error("Unknown error, changes to systemd?", zap.Error(err))
			}
		}
	}
}
