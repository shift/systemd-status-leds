package led

import (
	"sync"
)

type Led struct {
	sync.RWMutex
	Red    int64
	Green  int64
	Blue   int64
	White  int64
	Colour string
	Number int
	Unit   string
	Status string
}

func (l *Led) SetStatus(state string) {
	l.Status = state
}

func (l *Led) SetRed(r int64) {
	l.Red = r
}

func (l *Led) SetGreen(g int64) {
	l.Green = g
}

func (l *Led) SetBlue(b int64) {
	l.Blue = b
}

func (l *Led) SetWhite(w int64) {
	l.White = w
}

func (l *Led) SetColour(colour string) {
	l.Colour = colour
}
