package de.sve.backend.calendar;

import java.io.IOException;
import java.io.InputStream;
import java.security.GeneralSecurityException;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.util.Collections;
import java.util.Date;
import java.util.List;
import java.util.TimeZone;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.Collectors;

import com.google.api.client.googleapis.javanet.GoogleNetHttpTransport;
import com.google.api.client.json.jackson2.JacksonFactory;
import com.google.api.client.util.DateTime;
import com.google.api.services.calendar.Calendar;
import com.google.api.services.calendar.CalendarScopes;
import com.google.api.services.calendar.model.EventDateTime;
import com.google.api.services.calendar.model.Events;
import com.google.auth.http.HttpCredentialsAdapter;
import com.google.auth.oauth2.GoogleCredentials;

import de.sve.backend.model.calendar.Appointment;
import de.sve.backend.sheets.AbstractSheetController;

public class CalendarService {

	private static final String CREDENTIALS_FILE_PATH = "/credentials.json"; //$NON-NLS-1$

	private static final List<String> SCOPES = Collections.singletonList(CalendarScopes.CALENDAR_READONLY);

	private static final String TIME_ZONE = "Europe/Berlin"; //$NON-NLS-1$

	public static List<Appointment> appointments(String calendarId, int maxResults) throws GeneralSecurityException, IOException {
		DateTime timeMin = new DateTime(new Date(System.currentTimeMillis()), TimeZone.getTimeZone(TIME_ZONE));
		Events events = service().events()
        						 .list(calendarId)
        						 .setMaxResults(maxResults)
        						 .setTimeMin(timeMin)
        						 .setOrderBy("startTime") //$NON-NLS-1$
        						 .setSingleEvents(true)
        						 .execute();
		AtomicInteger sortIndexer = new AtomicInteger(0);
		return events.getItems().stream().map(item -> {
			EventDateTime start = item.getStart();
			EventDateTime end = item.getEnd();
			return Appointment.create(item.getId(),
									  sortIndexer.getAndIncrement(),
									  item.getSummary(),
									  item.getDescription(),
									  toLocalDate(start, 0),
									  toLocalDate(end, -1),
									  toLocalDateTime(start),
									  toLocalDateTime(end));
		}).collect(Collectors.toList());
	}

	private static LocalDate toLocalDate(EventDateTime eventDate, int daysToAdd) {
		if (eventDate == null || eventDate.getDate() == null) {
			return null;
		}
		return LocalDate.parse(eventDate.getDate().toStringRfc3339()).plusDays(daysToAdd);
	}

	private static LocalDateTime toLocalDateTime(EventDateTime eventDateTime) {
		if (eventDateTime == null || eventDateTime.getDateTime() == null) {
			return null;
		}
		return Instant.ofEpochMilli(eventDateTime.getDateTime().getValue())
					  .atZone(ZoneId.of(TIME_ZONE))
					  .toLocalDateTime();
	}

	private static Calendar service() throws GeneralSecurityException, IOException {
		return new Calendar.Builder(GoogleNetHttpTransport.newTrustedTransport(), JacksonFactory.getDefaultInstance(), credentials())
								 .setApplicationName("sve-backend-calendar-reader") //$NON-NLS-1$
								 .build();
	}

	private static HttpCredentialsAdapter credentials() throws IOException {
		try (InputStream inputStream = AbstractSheetController.class.getResourceAsStream(CREDENTIALS_FILE_PATH)) {
			return new HttpCredentialsAdapter(GoogleCredentials.fromStream(inputStream)
					 										   .createScoped(SCOPES));
		}
	}

}
