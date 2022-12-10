package main // github.com/shift/fcos-mc-pi4/leds

import (
	"fmt"
	systemd "github.com/coreos/go-systemd/v22/dbus" // change namespace
	systemdUtil "github.com/coreos/go-systemd/v22/util"
	"github.com/godbus/dbus/v5" // namespace collides with systemd wrapper
	"github.com/shift/fcos-mc-pi4/leds/led"
	"github.com/shift/fcos-mc-pi4/leds/strip"
	//"periph.io/x/conn/v3/physic"
	//"periph.io/x/conn/v3/spi"
	//"periph.io/x/conn/v3/spi/spireg"
	"periph.io/x/devices/v3/nrzled"
	//"periph.io/x/host/v3"

	"github.com/jar-o/limlog"
	"go.uber.org/zap"
	// "go.uber.org/zap/zapcore"
	"flag"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
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
	cfg.Encoding = "console" // By default this is JSON
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
	services := []string{"sshd.service", "minecraft.service", "local-exporter.service","zincati.service", "node-exporter.service"}
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
			fmt.Errorf("failed to get property: %+v", err)
			invalid = true
		}

		if !invalid {
			var notFound = (loadstate.Value == dbus.MakeVariant("not-found"))
			if notFound {
				fmt.Println("failed to find svc")
				invalid = true
			}
		}

		if previous != invalid { // if invalid changed, send signal
		}

		if invalid {
			fmt.Println("waiting fo service") // waiting for svc to appear...
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
					fmt.Printf("status: %+v\n", event[svc].Name)
					switch event[svc].ActiveState {
					case "active":
						fmt.Println("started")
						pixelRef.SetColour("00440005")

					case "inactive":
						fmt.Println("stopped")
						pixelRef.SetColour("44000005")
					case "reloading":
						fmt.Println("reloading")
						pixelRef.SetColour("60606060")
					case "failed":
						fmt.Println("failed")
						pixelRef.SetColour("99000000")
					case "activating":
						fmt.Println("activating")
						pixelRef.SetColour("00330010")
					case "deactivating":
						fmt.Println("deactivating")
						pixelRef.SetColour("22000010")
					default:
						fmt.Errorf("unknown svc state: %s", event[svc].ActiveState)
					}
				}

			case err := <-subErrors:
				fmt.Errorf("unknown %s error", err)
			}
		}
	}
}
