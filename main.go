package main // github.com/shift/systemd-status-leds

import (
	systemd "github.com/coreos/go-systemd/v22/dbus" // change namespace
	systemdUtil "github.com/coreos/go-systemd/v22/util"
	"github.com/godbus/dbus/v5" // namespace collides with systemd wrapper
	"github.com/shift/systemd-status-leds/led"
	"github.com/shift/systemd-status-leds/strip"

	"github.com/jar-o/limlog"
	"github.com/spf13/viper"
	"go.uber.org/zap"
)

type Service struct {
	Unit   string            `mapstructure:"name"`
	States map[string]string `mapstrcture:"states_map"`
}

type Config struct {
	Services []Service `mapstructure:"services"`
	Strip    struct {
		Length   int
		Channels int
		Hertz    int
		Spidev   string
		colours  map[string]string
	}
}

var (
	logr *limlog.Limlog
	C    Config
)

func Configuration() {
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")
	viper.AddConfigPath(".")
	err := viper.ReadInConfig()
	if err != nil {
		logr.Panic("config file", zap.Error(err))
	}
	err = viper.Unmarshal(&C)
	if err != nil {
		logr.Panic("config file", zap.Error(err))
	}
}

func main() {
	// First thigns first, logging...
	cfg := limlog.NewZapConfigWithLevel(zap.DebugLevel)
	logr = limlog.NewLimlogZapWithConfig(cfg)
	z := logr.L.GetLogger().(*zap.Logger)
	defer z.Sync()

	Configuration()
	z.Info("Strip",
		zap.String("spidev", C.Strip.Spidev),
		zap.Int("length", C.Strip.Length),
		zap.Int("channels", C.Strip.Channels),
		zap.Int("hertz", C.Strip.Hertz),
	)
	for _, service := range C.Services {
		z.Info("Service",
			zap.String("name", service.Unit),
		)
	}

	strip, err := strip.Init(logr, &C.Strip.Spidev, &C.Strip.Length, &C.Strip.Channels, &C.Strip.Hertz)

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
	for _, service := range C.Services {
		pixel, err := strip.Add(service.Unit)
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
				if event[svc] != nil {
					switch event[svc].ActiveState {
					case "active":
						pixelRef.SetColour(C.Strip.colours["active"])
					case "inactive":
						pixelRef.SetColour("44000005")
						pixelRef.SetColour(C.Strip.colours["inactive"])
					case "reloading":
						pixelRef.SetColour("60606060")
						pixelRef.SetColour(C.Strip.colours["reloading"])
					case "failed":
						pixelRef.SetColour("99000000")
						pixelRef.SetColour(C.Strip.colours["failed"])
					case "activating":
						pixelRef.SetColour("00330010")
						pixelRef.SetColour(C.Strip.colours["activating"])
					case "deactivating":
						pixelRef.SetColour("22000010")
						pixelRef.SetColour(C.Strip.colours["deactivating"])
					default:
						logr.Error("Unknown service statre", zap.String("event", event[svc].ActiveState))
					}
				}

			case err := <-subErrors:
				logr.Error("Unknown error, changes to systemd?", zap.Error(err))
			}
		}
	}
}
