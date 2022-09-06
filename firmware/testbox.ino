/* Hardware
 * 
 * BOM
 *   Wemos D1 mini
 *   DHT22 temp/humidity sensor
 *   LED
 *   Servo motor
 *                  _______________D1 mini_______________
 *                 |                                     |
 *                 | 1  RST                 /GPIO1 TX 22 |
 *                 | 2  A0 ADC0              GPIO3 RX 21 |
 *                 | 4  D0 GPIO16        SCL GPIO5 D1 20 |
 * Servo Motor <-- | 5  D5 GPIO14 SCK    SDA GPIO4 D2 19 |
 *   Green LED <-- | 6  D6 GPIO12 MISO       GPIO0 D3 18 | --> Red LED
 *  Yellow LED <-- | 7  D7 GPIO13 MOSI       GPIO2 D4 17 | --> DHT22 temp sensor
 *                 | 16 D8 GPIO15 SS              GND 15 |
 *                 | 8  3V3                       5V USB |
 *                 |_____________________________________|
  */

#include <map>

#include "DHTesp.h"
#include "Servo.h"

template<int MIN_V, int MAX_V, int DEF_V>
class ClampedInt {
private:
  int val_;

  static int clamped(int val) {
    return val > MAX_V ? MAX_V : (val < MIN_V ? MIN_V : val);
  }

public:
  static const int MIN = MIN_V;
  static const int MAX = MAX_V;
  static const int DEF = DEF_V;

  ClampedInt(int val) : val_(clamped(val))  {}
  ClampedInt()        : val_(DEF_V)         {}

  int get        ()                { return val_;                 }
  int set        (int val)         { return val_ = clamped(val);  }
  int operator = (const int & val) { return set(val);             }
};

typedef ClampedInt<0, 1023, 0> LedValue;
typedef ClampedInt<0, 180, 90> ServoValue;

struct TempSensorValue {
  unsigned long timestamp;
  const char *status;
  float temperature; // Celsius
  float humidity;    // Percent

  TempSensorValue()
  : timestamp(0), status(""), temperature(0), humidity(0)
  {}
};

template<int DHT_PIN_, int SERVO_PIN_, int RED_PIN_, int YELLOW_PIN_, int GREEN_PIN_>
class State {
private:
  DHTesp dht_;   // Temperature sensor handle
  Servo servo_;  // Servo motor handle

public:
  static const int DHT_PIN    = DHT_PIN_;
  static const int SERVO_PIN  = SERVO_PIN_;
  static const int RED_PIN    = RED_PIN_;
  static const int YELLOW_PIN = YELLOW_PIN_;
  static const int GREEN_PIN  = GREEN_PIN_;

  LedValue red, yellow, green;
  ServoValue servo;
  TempSensorValue sensor;

  void setup() {
    dht_.setup(DHT_PIN, DHTesp::DHT22);
    servo_.attach(SERVO_PIN);

    pinMode(RED_PIN,    OUTPUT);
    pinMode(YELLOW_PIN, OUTPUT);
    pinMode(GREEN_PIN,  OUTPUT);
  }

  void write() {
    analogWrite(RED_PIN,    red.get());
    analogWrite(YELLOW_PIN, yellow.get());
    analogWrite(GREEN_PIN,  green.get());
    servo_.write(servo.get());
  }

  void read() {
    unsigned long now = millis();

    // Skip reading if not enough time has passed
    if (now - sensor.timestamp < dht_.getMinimumSamplingPeriod())
      return;

    DHTesp::DHT_ERROR_t status = dht_.getStatus();

    sensor.timestamp = now;
    sensor.status = dht_.getStatusString();
    if (status == DHTesp::DHT_ERROR_t::ERROR_NONE) {
      sensor.temperature = dht_.getTemperature();
      sensor.humidity    = dht_.getHumidity();
    } else {
      sensor.temperature = 0;
      sensor.humidity    = 0;
    }
  }
};

// <DHT_PIN, SERVO_PIN, RED_PIN, YELLOW_PIN, GREEN_PIN>
typedef State<D4, D5, D3, D7, D6> TestBoxState;

class SelfTester {
public:
  struct Target {
    int red;
    int yellow;
    int green;
    int servo;
    int wait_ms;
  };
  
private:
  const std::vector<Target> targets_;
  bool active_;
  size_t step_;
  unsigned long next_step_timestamp_;

public:
  SelfTester(const std::vector<Target> & targets)
  : targets_(targets), active_(false), step_(0), next_step_timestamp_(0)
  {}

  bool is_active() const {
    return active_;
  }

  int progress() const {
    if (!active_ || targets_.size() == 0)
      return 0;

    return 100*step_/targets_.size();
  }

  void start() {
    if (active_)
      return; 
  
    active_ = true;
    step_ = 0;
    next_step_timestamp_ = millis();
  }

  void stop() {
    if (!active_)
      return;
    active_ = false;
  }

  template<typename S>
  bool step(S & state) {
    // Don't do anything if not self testing
    if (!active_)
      return false;
  
    // Check if we are done
    if (step_ >= targets_.size()) {
      stop();
      return false;
    }
  
    unsigned long now = millis();
  
    // Don't do anything if it is not time yet
    if (now < next_step_timestamp_)
      return false;
  
    unsigned long overshoot = next_step_timestamp_ - now;
  
    // Apply current state
    auto & target = targets_[step_];
    state.red    = target.red;
    state.yellow = target.yellow;
    state.green  = target.green;
    state.servo  = target.servo;
    state.write();
  
    // Schedule next step
    next_step_timestamp_ = now + target.wait_ms - overshoot;
    ++step_;

    return true;
  }
};

struct Request {
  enum Status {
    STATUS_OK,
    STATUS_BAD_SYNTAX
  } status;
  
  enum Verb {
    VERB_EMPTY,
    VERB_INVALID,
    VERB_ID,
    VERB_GET,
    VERB_SET,
  } verb;

  enum Noun {
    NOUN_EMPTY,
    NOUN_INVALID,
    NOUN_RED_LED,
    NOUN_YELLOW_LED,
    NOUN_GREEN_LED,
    NOUN_SERVO,
    NOUN_TEMP_AND_HUM,
    NOUN_SELF_TEST,
  } noun;

  enum Value {
    VALUE_EMPTY,
    VALUE_INVALID,
    VALUE_OK
  } value;

  int value_int;

  Request()
  : status(STATUS_OK), verb(Verb::VERB_EMPTY), noun(Noun::NOUN_EMPTY),
    value(Value::VALUE_EMPTY), value_int(0)
  {}
};


template<size_t MAX_LEN>
class RequestParser {  
private:
  char buffer_[MAX_LEN];
  size_t bufferp_;
  bool pending_request_;

public:
  RequestParser()
  : bufferp_(0), pending_request_(false)
  {}

  bool pending() const {
    return pending_request_;
  }

  bool available() const {
    return !pending() && bufferp_ < (sizeof(buffer_) - 1);
  }
  
  void push(char c) {
    if (available()) {
      buffer_[bufferp_++] = c;

      if (c == '\n' || bufferp_ == (sizeof(buffer_) - 1)) {
        buffer_[bufferp_] = '\0';
        bufferp_ = 0;
        pending_request_ = true;
      }
    }
  }

  bool parse_into(Request & request) {
    if (!pending())
      return false;

    pending_request_ = false;
    
    static const std::map<std::string, int> VERB_MAP = {
      {"ID",  Request::Verb::VERB_ID},
      {"GET", Request::Verb::VERB_GET},
      {"SET", Request::Verb::VERB_SET},
    };
    
    static const std::map<std::string, int> NOUN_MAP = {
      {"RED_LED",      Request::Noun::NOUN_RED_LED},
      {"YELLOW_LED",   Request::Noun::NOUN_YELLOW_LED},
      {"GREEN_LED",    Request::Noun::NOUN_GREEN_LED},
      {"SERVO",        Request::Noun::NOUN_SERVO},
      {"TEMP_AND_HUM", Request::Noun::NOUN_TEMP_AND_HUM},
      {"SELF_TEST",    Request::Noun::NOUN_SELF_TEST},
    };
    
    char *pch;
    pch = strtok(buffer_, " \n");

    // Parse VERB
    if (pch == NULL) {
      request.verb = Request::Verb::VERB_EMPTY;
      return true;
    }

    auto verb_it = VERB_MAP.find(std::string(pch));
    if (verb_it == VERB_MAP.end()) {
      request.verb = Request::Verb::VERB_INVALID;
      return true;
    }

    request.verb = (Request::Verb)verb_it->second;

    // Parse NOUN
    pch = strtok(NULL, " \r\n");
    if (pch == NULL) {
      request.noun = Request::Noun::NOUN_EMPTY;
      return true;
    }

    auto noun_it = NOUN_MAP.find(std::string(pch));
    if (noun_it == NOUN_MAP.end()) {
      Serial.print("Failed to find noun [");
      Serial.print(pch);
      Serial.println("]");
      request.noun = Request::Noun::NOUN_INVALID;
      return true;
    }
    
    request.noun = (Request::Noun)noun_it->second;

    // Parse VALUE
    pch = strtok(NULL, " \r\n");
    if (pch == NULL) {
      request.value = Request::Value::VALUE_EMPTY;
      return true;
    }

    char *value_end;
    request.value_int = strtol(pch, &value_end, 10);
    if (pch == value_end) {
      request.value = Request::Value::VALUE_INVALID;
      return true;
    }

    request.value = Request::Value::VALUE_OK;

    // Remaining tokens
    pch = strtok(NULL, "\r\n");
    if (pch != NULL) {
      request.value = Request::Value::VALUE_EMPTY;
      return true;
    }

    return true;
  }
};

typedef RequestParser<256> Parser;

static TestBoxState state;
static SelfTester self_tester({
  {state.red.DEF, state.yellow.DEF, state.green.DEF, state.servo.DEF, 500},
  {state.red.MAX, state.yellow.MIN, state.green.MIN, state.servo.MIN, 500},
  {state.red.MIN, state.yellow.MAX, state.green.MIN, state.servo.DEF, 500},
  {state.red.MIN, state.yellow.MIN, state.green.MAX, state.servo.MAX, 500},
  {state.red.DEF, state.yellow.DEF, state.green.DEF, state.servo.DEF, 500},
});
static Parser parser;

void setup() {
  Serial.begin(115200);

  state.setup();
  self_tester.start();
}

void handle_request(const Request & r) {
  using R = Request;
  
  if (r.status == R::STATUS_BAD_SYNTAX) {
    Serial.println("ERR BAD_SYNTAX");
    return;
  }

  if (r.verb == R::VERB_EMPTY || r.verb == R::VERB_INVALID) {
    Serial.println("ERR BAD_VERB");
    return;
  }

  switch (r.verb) {
    case R::VERB_ID: {
      if (r.noun != R::NOUN_EMPTY) {
        Serial.println("ERR BAD_NOUN");
      } else {
        Serial.print("OK ");
        Serial.println(ARDUINO_BOARD);
      }
      return;
    }

    case R::VERB_GET: {
      if (r.value != R::VALUE_EMPTY) {
        Serial.println("ERR BAD_VALUE");
        return;
      }
      
      switch (r.noun) {
        case R::NOUN_RED_LED:
          Serial.print("OK ");
          Serial.println(state.red.get());
          return;
          
        case R::NOUN_YELLOW_LED:
          Serial.print("OK ");
          Serial.println(state.yellow.get());
          return;

        case R::NOUN_GREEN_LED:
          Serial.print("OK ");
          Serial.println(state.green.get());
          return;

        case R::NOUN_SERVO:
          Serial.print("OK ");
          Serial.println(state.servo.get());
          return;

        case R::NOUN_TEMP_AND_HUM:
          Serial.print("OK ");
          Serial.print(state.sensor.status);
          Serial.print(" ");
          Serial.print(state.sensor.temperature);
          Serial.print(" ");
          Serial.println(state.sensor.humidity);
          return;
          
        case R::NOUN_SELF_TEST:
          Serial.print("OK ");
          Serial.print(self_tester.is_active() ? "ACTIVE" : "INACTIVE");
          Serial.print(" ");
          Serial.println(self_tester.progress());
          return;

        default:
          Serial.println("ERR BAD_NOUN");
          return;
      }
      return;
    }

    case R::VERB_SET: {
      if (r.value != R::VALUE_OK) {
        Serial.println("ERR BAD_VALUE");
        return;
      }
      
      switch (r.noun) {
        case R::NOUN_RED_LED:
          state.red = r.value_int;
          state.write();
          Serial.print("OK ");
          Serial.println(state.red.get());
          return;
          
        case R::NOUN_YELLOW_LED:
          state.yellow = r.value_int;
          state.write();
          Serial.print("OK ");
          Serial.println(state.yellow.get());
          return;

        case R::NOUN_GREEN_LED:
          state.green = r.value_int;
          state.write();
          Serial.print("OK ");
          Serial.println(state.green.get());
          return;

        case R::NOUN_SERVO:
          state.servo = r.value_int;
          state.write();
          Serial.print("OK ");
          Serial.println(state.servo.get());
          return;

        case R::NOUN_SELF_TEST:
          if (r.value_int < 0 || r.value_int > 1) {
            Serial.println("ERR BAD_VALUE");
          } else {
            if (r.value_int)
              self_tester.start();
            else
              self_tester.stop();
  
            Serial.print("OK ");
            Serial.print(self_tester.is_active() ? "ACTIVE" : "INACTIVE");
            Serial.print(" ");
            Serial.println(self_tester.progress());
          }
          return;

        default:
          Serial.println("ERR BAD_NOUN");
          return;
      }
      return;
    }

    default: {
      Serial.println("ERR BAD_VERB");
      return;
    }
  }
}

void loop() {
  static unsigned long refresh_delay = 20;
  static Request request;

  unsigned long loop_start = millis();

  self_tester.step(state);
  state.read();

  while (Serial.available() && parser.available())
    parser.push(Serial.read());

  if (parser.parse_into(request))
    handle_request(request);

  unsigned long loop_end = millis();
  unsigned long elapsed = loop_end - loop_start;

  if (elapsed < refresh_delay)
    delay(refresh_delay - elapsed);
}
