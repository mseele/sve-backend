package de.sve.backend.manager;

import static java.util.Objects.requireNonNullElse;

import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.text.NumberFormat;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Map.Entry;
import java.util.Objects;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Collectors;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.gson.Gson;

import de.sve.backend.Utils;
import de.sve.backend.mail.Mail;
import de.sve.backend.mail.MailAccount;
import de.sve.backend.model.events.BookingResponse;
import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventBooking;
import de.sve.backend.model.events.EventCounter;
import de.sve.backend.model.events.EventType;
import de.sve.backend.model.news.NewsType;
import de.sve.backend.model.news.Subscription;
import de.sve.backend.sheets.EventsSheetController;
import de.sve.backend.sheets.SheetExtractor;
import de.sve.backend.store.DataStore;

public class EventsManager {

	private static final Logger LOG = LoggerFactory.getLogger(EventsManager.class);

	private static final Locale DE = Locale.GERMANY;

	private static DateTimeFormatter DATE_FORMAT = DateTimeFormatter.ofPattern("E, dd. MMM yyyy, HH:mm", DE); //$NON-NLS-1$

	private static DateTimeFormatter PAYDAY_FORMAT = DateTimeFormatter.ofPattern("dd. MMMM", DE); //$NON-NLS-1$

	private static NumberFormat PRICE_FORMAT = NumberFormat.getCurrencyInstance(DE);

	private static Pattern PAYDAY_PATTERN = Pattern.compile("\\Q${payday:\\E(?<days>\\d+)\\Q}\\E"); //$NON-NLS-1$

	private static String MESSAGE_FAIL = "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal."; //$NON-NLS-1$

	public static List<Event> events(boolean beta) throws Exception {
		return events(beta, null);
	}

	public static List<Event> events(Boolean beta, EventType type) throws Exception {
		return DataStore.events()
				.stream()
		 		.filter(e -> e.visible() && (type == null || type == e.type()) && (beta == null || beta.booleanValue() == e.beta()))
		 		.sorted((event1, event2) -> {
					 if (event1.isBookedUp() == event2.isBookedUp()) {
						 return event1.sortIndex().compareTo(event2.sortIndex());
					 } else if (event1.isBookedUp()) {
						 return 1;
					 }
					 return -1;
				 })
		 		.collect(Collectors.toList());
	}

	public static List<EventCounter> eventsCounter() throws Exception {
		return events(null, null).stream()
					   			 .map(EventCounter::create)
					   			 .collect(Collectors.toList());
	}

	public static BookingResponse prebooking(String hash) {
		Charset chartset = StandardCharsets.UTF_8;
		String decoded = new String(Base64.getDecoder().decode(hash.getBytes(chartset)), chartset);
		String[] splitted = decoded.split("#"); //$NON-NLS-1$
		if (splitted.length == 8) {
			EventBooking booking = EventBooking.create(splitted[0],
					   								   splitted[1],
					   								   splitted[2],
					   								   splitted[3],
					   								   splitted[4],
					   								   splitted[5],
					   								   splitted[6],
					   								   Boolean.valueOf("J".equals(splitted[7])), //$NON-NLS-1$
					   								   Boolean.FALSE,
													   "Pre-Booking"); //$NON-NLS-1$
			BookingResponse checkResult = checkPrebooking(booking);
			if (checkResult != null) {
				return booking(booking);
			}
			return checkResult;
		}
		LOG.error("Booking failed beacuse spitted prebooking hash (" + decoded + ") has an invalid length:" + Arrays.asList(splitted)); //$NON-NLS-1$ //$NON-NLS-2$
		return BookingResponse.failure(MESSAGE_FAIL);
	}

	private static BookingResponse checkPrebooking(EventBooking booking) {
		try {
			Event event = DataStore.event(booking.eventId());
			SheetExtractor sheetExtractor = new SheetExtractor(event.sheetId());
			Map<String, String> accessor = new HashMap<>();
			accessor.put("Vorname", booking.firstName()); //$NON-NLS-1$
			accessor.put("Nachname", booking.lastName()); //$NON-NLS-1$
			accessor.put("Straße & Nr", booking.street()); //$NON-NLS-1$
			accessor.put("PLZ & Ort", booking.city()); //$NON-NLS-1$
			accessor.put("Email", booking.email()); //$NON-NLS-1$
			accessor.put("Telefon", booking.phone()); //$NON-NLS-1$
			accessor.put("SVE-Mitglied", booking.isMember() ? "J" : "N"); //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
			List<Map<String, String>> rows = sheetExtractor.get(event.gid().intValue(), new ArrayList<>(accessor.keySet()));
			for (Map<String, String> row : rows) {
				boolean match = true;
				for (Entry<String, String> cell : row.entrySet()) {
					String value = accessor.get(cell.getKey());
					String cellValue = cell.getValue();
					if (!Objects.equals(requireNonNullElse(value, "").strip(),	 //$NON-NLS-1$
										requireNonNullElse(cellValue, "").strip())) { //$NON-NLS-1$
						match = false;
						break;
					}

				}
				if (match) {
					LOG.warn("Prebooking link data has been detected and invalidated for booking " + booking); //$NON-NLS-1$
					return BookingResponse.failure("Der Buchungslink wurde schon benutzt und ist daher ungültig."); //$NON-NLS-1$
				}
			}
			return null;
		} catch (Throwable t) {
			LOG.error("Prebooking check failed", t); //$NON-NLS-1$
			return BookingResponse.failure(MESSAGE_FAIL);
		}
	}

	public static BookingResponse booking(EventBooking booking) {
		try {
			Event event = DataStore.event(booking.eventId());
			if (event.maxSubscribers() == -1 || event.subscribers() < event.maxSubscribers()) {
				return successfullBooking(booking, event.bookEvent(), true);
			} else if (event.waitingList() < event.maxWaitingList()) {
				return successfullBooking(booking, event.bookEvent(), false);
			}
			LOG.error("Booking failed because Event (" + event.id() + ") was overbooked."); //$NON-NLS-1$ //$NON-NLS-2$
			return BookingResponse.failure(MESSAGE_FAIL);
		} catch (Throwable t) {
			LOG.error("Booking failed", t); //$NON-NLS-1$
			return BookingResponse.failure(MESSAGE_FAIL);
		}
	}

	private static BookingResponse successfullBooking(EventBooking booking, Event event, boolean isBooking) throws Throwable {
		String result = EventsSheetController.saveBooking(booking, event);
		if (booking.subscribeUpdates()) {
			NewsType type = event.type() == EventType.Events ? NewsType.Events : NewsType.Fitness;
			Subscription subscription = Subscription.create(booking.email(), type);
			NewsManager.subscribe(subscription, false);
		}
		sendMail(booking, event, isBooking);
		DataStore.save(event);
		LOG.info("Booking of Event (" + event.id() + ") was successfull: " + result); //$NON-NLS-1$ //$NON-NLS-2$
		String message;
		if (isBooking) {
			message = "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail."; //$NON-NLS-1$
		} else {
			message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden."; //$NON-NLS-1$
		}
		return BookingResponse.success(message, eventsCounter());
	}

	private static void sendMail(EventBooking booking, Event event, boolean isBooking) throws Throwable {
		try {
			EventType type = event.type();
			MailAccount account = MailAccount.of(type);
			Mail.Builder builder = Mail.via(account);
			String subjectPrefix;
			switch (type) {
				case Fitness:
					subjectPrefix = "[Fitness"; //$NON-NLS-1$
					break;
				case Events:
					subjectPrefix = "[Events"; //$NON-NLS-1$
					break;
				default:
					throw new IllegalArgumentException("Type '" + type + "' is not supported");  //$NON-NLS-1$//$NON-NLS-2$
			}
			subjectPrefix += "@SVE]"; //$NON-NLS-1$
			String template;
			if (isBooking) {
				builder.subject(subjectPrefix + " Bestätigung Buchung"); //$NON-NLS-1$
				template = event.bookingTemplate();
			} else {
				builder.subject(subjectPrefix + " Bestätigung Warteliste"); //$NON-NLS-1$
				template = event.waitingTemplate();
			}

			StringBuilder content = new StringBuilder(replace(template, booking, event));
			if (booking.subscribeUpdates()) {
				String typeName = type == EventType.Events ? "Events" : "Kursangebote"; //$NON-NLS-1$ //$NON-NLS-2$
				content.append("\n\nPS: Ab sofort erhältst Du automatisch eine E-Mail, sobald neue " + typeName + " online sind.\n" + //$NON-NLS-1$ //$NON-NLS-2$
							   "\n" + //$NON-NLS-1$
							   NewsManager.unsubscribeAppendix());
			}
			builder.content(content.toString())
				   .to(booking.email())
				   .bcc(account.email())
				   .send();

			LOG.info("Booking email was send successfully"); //$NON-NLS-1$
		} catch (Throwable e) {
			Gson gson = Utils.gson();
			String message = "Error while sending mail for booking (" + gson.toJson(booking) + ") of event (" + gson.toJson(event) + ")."; //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
			LOG.error(message, e);
			throw e;
		}
	}

	private static final String replace(String template, EventBooking booking, Event event) {
		String content = template.replace("${firstname}", booking.firstName().trim()); //$NON-NLS-1$
		content = content.replace("${lastname}", booking.lastName().trim()); //$NON-NLS-1$
		content = content.replace("${name}", event.name().trim()); //$NON-NLS-1$
		content = content.replace("${location}", event.location()); //$NON-NLS-1$
		StringBuilder dates = new StringBuilder();
		String paydayToReplace = "${payday}"; //$NON-NLS-1$
		int days = 14;
		Matcher matcher = PAYDAY_PATTERN.matcher(content);
		if (matcher.find()) {
			paydayToReplace = content.substring(matcher.start(), matcher.end());
			days = Integer.parseInt(matcher.group("days")); //$NON-NLS-1$
		}
		LocalDateTime payday = null;
		for (LocalDateTime date : event.dates()) {
			if (dates.length() > 0) {
				dates.append("\n"); //$NON-NLS-1$
			}
			dates.append("- "); //$NON-NLS-1$
			dates.append(DATE_FORMAT.format(date));
			dates.append(" Uhr"); //$NON-NLS-1$
			if (payday == null) {
				payday = date.minusDays(days);
				if (payday.isBefore(LocalDateTime.now())) {
					payday = LocalDateTime.now();
				}
			}
		}
		content = content.replace("${dates}", dates.toString()); //$NON-NLS-1$
		if (payday != null) {
			content = content.replace(paydayToReplace, PAYDAY_FORMAT.format(payday));
		}
		Double cost = booking.cost(event);
		String price = PRICE_FORMAT.format(cost);
		return content.replace("${price}", price); //$NON-NLS-1$
	}

	public static Event update(Event event) throws Exception {
		DataStore.save(event);
		return DataStore.event(event.id());
	}

	public static void delete(Event event) throws Exception {
		DataStore.delete(event);
	}

}
