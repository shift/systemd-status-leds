package strip

import (
	"bytes"
	"errors"
	"fmt"
	"strconv"
	"github.com/shift/fcos-mc-pi4/leds/led"
	"periph.io/x/conn/v3/physic"
	"periph.io/x/conn/v3/spi"
	"periph.io/x/conn/v3/spi/spireg"
	"periph.io/x/devices/v3/nrzled"
	"periph.io/x/host/v3"
	"time"
)

var (
	Loading = []byte{60, 60, 60, 60}
)

type Strip struct {
	SPIBus   *string
	HRz      physic.Frequency
	Channels *int
	Count    *int
	Display  *nrzled.Dev
	Pixels   []*led.Led
	spidev   spi.PortCloser
}

func Init(spiBus *string, hrz physic.Frequency, count *int, channels *int) (*Strip, error) {

	strip := &Strip{}
	strip.SPIBus = spiBus
	strip.HRz = hrz
	strip.Count = count
	strip.Channels = channels

	if _, err := host.Init(); err != nil {
		return nil, errors.New("Unable to intialize the pariph.Host.")
	}

	var err error
	if strip.spidev, err = spireg.Open(*strip.SPIBus); err != nil {
		return nil, err
	}
	//defer s.Close()

	if p, ok := strip.spidev.(spi.Pins); ok {
		fmt.Printf("Using pins CLK: %s  MOSI: %s  MISO: %s", p.CLK(), p.MOSI(), p.MISO())
	}
	o := nrzled.Opts{
		NumPixels: *strip.Count,
		Channels:  *strip.Channels,
		Freq:      2500 * physic.KiloHertz,
	}
	strip.Display, err = nrzled.NewSPI(strip.spidev, &o)
	if err != nil {
		return nil, err
	}
	ledlen, err := strip.Display.Write(bytes.Repeat(Loading, *strip.Count-1))
	if err != nil {
		fmt.Println(err)
	} else {
		fmt.Println(ledlen)
	}

	return strip, nil
}

func (strip *Strip) Add(unit string) (pixel *led.Led, err error) {
	led := &led.Led{}
	led.Unit = unit

	if len(strip.Pixels) == *strip.Count {
		return nil, errors.New("Already at one service per pixel.")
	} else {
		strip.Pixels = append(strip.Pixels, led)
		led.Number = len(strip.Pixels)
		return led, nil
	}
	return led, nil
}

func (s *Strip) UpdateLoop() {
	buf := make([]byte, 5*4)
	for {
			fmt.Println("start")
		for _, p := range s.Pixels {
			offset := (p.Number-1)*4
			rgba, _ := strconv.ParseUint(p.Colour, 16, 32)
			buf[offset] = byte(rgba >> 24)
			buf[offset+1] = byte(rgba >> 16)
			buf[offset+2] = byte(rgba >> 8)
			buf[offset+3] = byte(rgba)
		}
			fmt.Println("end")
		fmt.Println(buf)
		_, _ = s.Display.Write(buf)
		time.Sleep(5 * time.Second)
	}
}
